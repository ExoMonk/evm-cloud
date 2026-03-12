# k3s Bootstrap — installs k3s on a remote host via SSH.
# Provider-agnostic: works with any host (AWS EC2, bare metal VPS, etc.)
#
# Lifecycle:
#   create  → install k3s + readiness check + extract kubeconfig
#   destroy → run k3s-uninstall.sh (removes k3s + all workloads)
#
# Recovery: if provisioner fails mid-install, run:
#   terraform taint module.<path>.null_resource.k3s_install
# Then re-apply. The k3s install script is idempotent.

locals {
  # Build TLS SAN flags: always include host_address, plus user-provided entries
  all_tls_sans = distinct(concat([var.host_address], var.tls_san_entries))
  tls_san_flags = join(" ", [
    for san in local.all_tls_sans : "--tls-san ${san}"
  ])

  node_name = "${var.project_name}-server-0"
}

resource "null_resource" "k3s_install" {
  triggers = {
    k3s_version        = var.k3s_version
    tls_san_entries    = join(",", local.all_tls_sans)
    extra_server_flags = var.extra_server_flags
    host_address       = var.host_address
    ssh_user           = var.ssh_user
    ssh_private_key    = var.ssh_private_key_path
    ssh_port           = var.ssh_port
    # NOTE: project_name intentionally excluded — k3s is per-host, not per-project.
    # Multiple projects can share one k3s cluster on the same host.
  }

  connection {
    type        = "ssh"
    host        = self.triggers.host_address
    user        = self.triggers.ssh_user
    private_key = file(self.triggers.ssh_private_key)
    port        = self.triggers.ssh_port
    timeout     = "2m"
  }

  # Install k3s with security hardening (idempotent — skips if already running)
  provisioner "remote-exec" {
    inline = [
      "set -eu",

      # Skip install if k3s is already running on this host
      "if systemctl is-active --quiet k3s 2>/dev/null; then",
      "  echo '[evm-cloud] k3s is already running on this host, skipping install.'",
      "  echo '[evm-cloud] Ensuring kubeconfig is available...'",
      "  mkdir -p $HOME/.kube",
      "  sudo cp /etc/rancher/k3s/k3s.yaml $HOME/.kube/k3s-kubeconfig.yaml",
      "  sudo chown $(whoami) $HOME/.kube/k3s-kubeconfig.yaml",
      "  chmod 0600 $HOME/.kube/k3s-kubeconfig.yaml",
      "  sed -i \"s|127.0.0.1|${var.host_address}|g\" $HOME/.kube/k3s-kubeconfig.yaml",
      "  exit 0",
      "fi",

      "echo '[evm-cloud] Installing k3s ${var.k3s_version}...'",

      # Download k3s binary and verify checksum
      "K3S_VERSION='${var.k3s_version}'",
      "ARCH=$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/')",
      "curl -sfL -o /tmp/k3s \"https://github.com/k3s-io/k3s/releases/download/$${K3S_VERSION}/k3s$${ARCH:+$([ \"$ARCH\" = 'arm64' ] && echo '-arm64' || echo '')}\"",
      "curl -sfL -o /tmp/k3s-checksums \"https://github.com/k3s-io/k3s/releases/download/$${K3S_VERSION}/sha256sum-$${ARCH}.txt\"",
      "EXPECTED=$(grep -E ' k3s$| k3s-arm64$' /tmp/k3s-checksums | awk '{print $1}' | head -1)",
      "ACTUAL=$(sha256sum /tmp/k3s | awk '{print $1}')",
      "if [ \"$EXPECTED\" != \"$ACTUAL\" ]; then echo 'ERROR: k3s checksum verification failed'; exit 1; fi",
      "echo '[evm-cloud] Checksum verified.'",

      # Install k3s with pre-verified binary
      "sudo chmod +x /tmp/k3s",
      "sudo cp /tmp/k3s /usr/local/bin/k3s",

      # Write k3s config file before install:
      # 1. DNS fix: Ubuntu uses systemd-resolved stub (127.0.0.53) which is unreachable from pods.
      # 2. CIDR fix: k3s defaults to 10.42.0.0/16 for pods, which often collides with VPC CIDRs.
      "sudo mkdir -p /etc/rancher/k3s",
      "sudo tee /etc/rancher/k3s/config.yaml > /dev/null <<CONF",
      "cluster-cidr: \"${var.cluster_cidr}\"",
      "service-cidr: \"${var.service_cidr}\"",
      "node-label:",
      "  - evm-cloud/role=server",
      "CONF",
      "if [ -f /run/systemd/resolve/resolv.conf ]; then echo 'resolv-conf: \"/run/systemd/resolve/resolv.conf\"' | sudo tee -a /etc/rancher/k3s/config.yaml > /dev/null; fi",

      # Use hostname for node name so it's stable across projects on the same host
      "NODE_NAME=$(hostname | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9-]/-/g')-server-0",
      "curl -sfL https://get.k3s.io | INSTALL_K3S_SKIP_DOWNLOAD=true INSTALL_K3S_VERSION='${var.k3s_version}' sh -s - server ${local.tls_san_flags} --disable=traefik --disable=servicelb --secrets-encryption --write-kubeconfig-mode=0600 --node-name $NODE_NAME ${var.extra_server_flags}",

      # Readiness check — wait for k3s API server
      "echo '[evm-cloud] Waiting for k3s to become ready...'",
      "for i in $(seq 1 60); do sudo k3s kubectl get nodes --no-headers 2>/dev/null | grep -q ' Ready' && break; sleep 2; done",
      "sudo k3s kubectl get nodes --no-headers | grep -q ' Ready' || { echo 'ERROR: k3s failed to become ready within 120s'; exit 1; }",
      "echo '[evm-cloud] k3s is ready.'",

      # Extract kubeconfig with external endpoint (persistent path, survives reboot)
      "mkdir -p $HOME/.kube",
      "sudo cp /etc/rancher/k3s/k3s.yaml $HOME/.kube/k3s-kubeconfig.yaml",
      "sudo chown $(whoami) $HOME/.kube/k3s-kubeconfig.yaml",
      "chmod 0600 $HOME/.kube/k3s-kubeconfig.yaml",
      "sed -i \"s|127.0.0.1|${var.host_address}|g\" $HOME/.kube/k3s-kubeconfig.yaml",
      "echo '[evm-cloud] k3s install complete.'",
    ]
  }

  # Teardown: drain pods, clean up CNI network interfaces, then remove k3s.
  # This must happen before EC2 termination — otherwise flannel/CNI-created ENIs
  # orphan in the VPC and block IGW/subnet/VPC deletion for 10+ minutes.
  provisioner "remote-exec" {
    when       = destroy
    on_failure = continue
    inline = [
      "echo '[evm-cloud] Draining k3s node and cleaning up...'",

      # Delete all non-system workloads so CNI releases their network interfaces
      "if command -v k3s >/dev/null 2>&1; then sudo k3s kubectl delete --all deployments,statefulsets,daemonsets,jobs,pods --all-namespaces --ignore-not-found --timeout=60s 2>/dev/null || true; fi",

      # Wait for CNI to release ENIs
      "sleep 10",

      # Uninstall k3s (removes flannel interfaces, CNI configs, iptables rules)
      "echo '[evm-cloud] Uninstalling k3s...'",
      "if [ -f /usr/local/bin/k3s-uninstall.sh ]; then sudo /usr/local/bin/k3s-uninstall.sh; fi",

      # Clean up any leftover CNI interfaces and bridges
      "sudo ip link delete flannel.1 2>/dev/null || true",
      "sudo ip link delete cni0 2>/dev/null || true",

      # Wait for AWS to fully release ENIs before EC2 termination.
      # Flannel CNI creates secondary ENIs in the VPC. Even after k3s-uninstall
      # and interface deletion, AWS takes 15-30s to fully deregister them.
      # Without this, IGW/subnet/VPC destroy hangs waiting for ENI detachment.
      "echo '[evm-cloud] Waiting for AWS ENI cleanup (30s)...'",
      "sleep 30",

      "rm -f $HOME/.kube/k3s-kubeconfig.yaml /tmp/k3s /tmp/k3s-checksums",
      "echo '[evm-cloud] k3s uninstall complete.'",
    ]
  }
}

# --- Extract secrets via local-exec provisioners ---
# Writes to local files after install. No SSH during plan or destroy.
# Uses external data sources (not file()) to read values — external with
# depends_on defers execution to apply time, so values are available in
# state after a single `terraform apply` (no refresh needed).

locals {
  secrets_dir     = "${path.root}/.evm-cloud/secrets"
  token_file      = "${local.secrets_dir}/${var.project_name}.node-token"
  kubeconfig_file = "${local.secrets_dir}/${var.project_name}.kubeconfig.b64"
}

resource "terraform_data" "fetch_secrets" {
  depends_on       = [null_resource.k3s_install]
  triggers_replace = [null_resource.k3s_install.id]

  # Fetch node token (clear stale files first — server may have been recreated)
  provisioner "local-exec" {
    command = <<-EOF
      rm -rf ${local.secrets_dir}
      mkdir -p ${local.secrets_dir}
      for i in $(seq 1 10); do
        TOKEN=$(ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=10 \
          -i ${var.ssh_private_key_path} -p ${var.ssh_port} \
          ${var.ssh_user}@${var.host_address} \
          "sudo cat /var/lib/rancher/k3s/server/node-token 2>/dev/null") && [ -n "$TOKEN" ] && break
        sleep 3
      done
      if [ -z "$TOKEN" ]; then echo "ERROR: Failed to retrieve k3s node token"; exit 1; fi
      printf '%s' "$TOKEN" > ${local.token_file}
    EOF
  }

  # Fetch kubeconfig
  provisioner "local-exec" {
    command = <<-EOF
      RAW=$(ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        -p ${var.ssh_port} -i ${var.ssh_private_key_path} \
        ${var.ssh_user}@${var.host_address} \
        'cat $HOME/.kube/k3s-kubeconfig.yaml')
      KUBECONFIG_B64=$(echo "$RAW" | base64 -w0 2>/dev/null || echo "$RAW" | base64 | tr -d '\n')
      printf '%s' "$KUBECONFIG_B64" > ${local.kubeconfig_file}
    EOF
  }
}

# --- Read secrets from local files (deferred to apply time) ---
# external data sources with depends_on are evaluated during apply, not plan.
# This ensures values are in state after a single `terraform apply`.

data "external" "kubeconfig" {
  depends_on = [terraform_data.fetch_secrets]
  program = ["bash", "-c", <<-PROG
    FILE='${local.kubeconfig_file}'
    if [ -f "$FILE" ]; then
      jq -n --arg v "$(cat "$FILE")" '{"value":$v}'
    else
      echo '{"value":""}'
    fi
  PROG
  ]
}

data "external" "node_token" {
  depends_on = [terraform_data.fetch_secrets]
  program = ["bash", "-c", <<-PROG
    FILE='${local.token_file}'
    if [ -f "$FILE" ]; then
      jq -n --arg v "$(cat "$FILE")" '{"value":$v}'
    else
      echo '{"value":""}'
    fi
  PROG
  ]
}
