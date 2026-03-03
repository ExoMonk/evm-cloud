# k3s Agent — joins worker nodes to an existing k3s server cluster.
# Provider-agnostic: works with any SSH-reachable host (AWS EC2, bare metal VPS, etc.)
#
# Lifecycle:
#   create  → write config + install k3s agent + readiness check (node joins cluster)
#   destroy → drain node + delete from cluster + uninstall k3s agent
#
# Recovery: if provisioner fails mid-install, run:
#   terraform taint 'module.<path>.null_resource.k3s_agent_config["<node-name>"]'
#   terraform taint 'module.<path>.null_resource.k3s_agent["<node-name>"]'
# Then re-apply. The k3s install script is idempotent.

# Stage 1: Write k3s agent config (token, server URL, labels) to the worker host.
# Separate resource so the sensitive token doesn't suppress k3s_agent provisioner output.
# Config persists at /etc/rancher/k3s/config.yaml — survives systemd restarts.
resource "null_resource" "k3s_agent_config" {
  for_each = { for node in var.worker_nodes : node.name => node }

  triggers = {
    host                 = each.value.host
    ssh_user             = each.value.ssh_user
    ssh_private_key_path = each.value.ssh_private_key_path
    ssh_port             = each.value.ssh_port
    server_host          = var.server_host
    node_name            = "${var.project_name}-worker-${each.value.name}"
    role                 = each.value.role
    token_hash           = sha256(var.node_token)
  }

  connection {
    type        = "ssh"
    host        = self.triggers.host
    user        = self.triggers.ssh_user
    private_key = file(self.triggers.ssh_private_key_path)
    port        = self.triggers.ssh_port
    timeout     = "2m"
  }

  # Write token to temp file, then build config.yaml from it
  provisioner "file" {
    content     = var.node_token
    destination = "/tmp/k3s-token"
  }

  provisioner "remote-exec" {
    inline = [
      "set -eu",
      "sudo mkdir -p /etc/rancher/k3s",
      "TOKEN=$(cat /tmp/k3s-token)",
      "rm -f /tmp/k3s-token",
      "sudo tee /etc/rancher/k3s/config.yaml > /dev/null <<CONF",
      "server: https://${self.triggers.server_host}:6443",
      "token: $TOKEN",
      "node-name: ${self.triggers.node_name}",
      "node-label:",
      "  - evm-cloud/role=${self.triggers.role}",
      "CONF",
      "echo '[evm-cloud] Agent config written to /etc/rancher/k3s/config.yaml'",
    ]
  }

}

# Stage 2: Install k3s agent — config already on disk, full output visible.
resource "null_resource" "k3s_agent" {
  for_each   = { for node in var.worker_nodes : node.name => node }
  depends_on = [null_resource.k3s_agent_config]

  triggers = {
    k3s_version = var.k3s_version
    server_host = var.server_host
    host        = each.value.host
    role        = each.value.role
    node_name   = "${var.project_name}-worker-${each.value.name}"
    config_id   = null_resource.k3s_agent_config[each.key].id

    # Connection + readiness check need these via self.triggers
    ssh_user             = each.value.ssh_user
    ssh_private_key_path = each.value.ssh_private_key_path
    ssh_port             = each.value.ssh_port
    server_ssh_user      = var.server_ssh_user
    server_ssh_key       = var.server_ssh_private_key_path
    server_ssh_port      = var.server_ssh_port
  }

  connection {
    type        = "ssh"
    host        = self.triggers.host
    user        = self.triggers.ssh_user
    private_key = file(self.triggers.ssh_private_key_path)
    port        = self.triggers.ssh_port
    timeout     = "2m"
  }

  # Install k3s agent — reads config from /etc/rancher/k3s/config.yaml
  provisioner "remote-exec" {
    inline = [
      "set -eu",
      "echo '[evm-cloud] Installing k3s agent on ${self.triggers.node_name}...'",
      "curl -sfL https://get.k3s.io | INSTALL_K3S_VERSION='${self.triggers.k3s_version}' sh -s - agent || { echo '[evm-cloud] k3s agent service failed — dumping journal:'; sudo journalctl -u k3s-agent --no-pager -n 30; exit 1; }",
      "echo '[evm-cloud] k3s agent installed.'",
    ]
  }

  # Post-join readiness check — SSH to server, poll for node to appear, then wait for Ready
  provisioner "local-exec" {
    command = <<-EOF
      ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=10 \
        -i ${self.triggers.server_ssh_key} -p ${self.triggers.server_ssh_port} \
        ${self.triggers.server_ssh_user}@${self.triggers.server_host} bash -s <<'SCRIPT'
      set -eu
      NODE="${self.triggers.node_name}"
      ROLE="${self.triggers.role}"

      # Wait for node to register (kubectl wait fails immediately if node doesn't exist)
      echo "[evm-cloud] Waiting for node $NODE to register..."
      for i in $(seq 1 60); do
        sudo k3s kubectl get node "$NODE" --no-headers 2>/dev/null && break
        sleep 2
      done

      # Wait for Ready condition
      sudo k3s kubectl wait --for=condition=Ready "node/$NODE" --timeout=120s

      # Verify label
      if sudo k3s kubectl get node "$NODE" --show-labels | grep -q "evm-cloud/role=$ROLE"; then
        echo "[evm-cloud] Node $NODE Ready with label evm-cloud/role=$ROLE"
      else
        echo "[evm-cloud] WARNING: label evm-cloud/role=$ROLE not found, applying manually..."
        sudo k3s kubectl label node "$NODE" "evm-cloud/role=$ROLE" --overwrite
      fi
      SCRIPT
    EOF
  }

  # No destroy provisioners — EC2 termination handles cleanup.
  # Server's k3s-uninstall.sh removes the full cluster on terraform destroy.
}
