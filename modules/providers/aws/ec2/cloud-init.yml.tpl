#cloud-config
package_update: true

packages:
  - docker
  - jq

runcmd:
  - systemctl enable docker
  - systemctl start docker
  # Install Docker Compose plugin
  - mkdir -p /usr/local/lib/docker/cli-plugins
  - curl -SL "https://github.com/docker/compose/releases/latest/download/docker-compose-linux-x86_64" -o /usr/local/lib/docker/cli-plugins/docker-compose
  - chmod +x /usr/local/lib/docker/cli-plugins/docker-compose
  # Create working directories and set ownership for ec2-user (SCP access)
  - mkdir -p /opt/evm-cloud/config/abis /opt/evm-cloud/scripts
  - chown -R ec2-user:ec2-user /opt/evm-cloud
%{ if workload_mode == "terraform" ~}
  # Pull secrets and start services
  - bash /opt/evm-cloud/scripts/pull-secrets.sh
  - cd /opt/evm-cloud && docker compose --env-file .env up -d
%{ endif ~}

write_files:
  # Pull secrets script
  - path: /opt/evm-cloud/scripts/pull-secrets.sh
    permissions: '0755'
    content: |
      ${indent(6, pull_secrets_script)}

%{ if workload_mode == "terraform" ~}
  # Docker Compose file
  - path: /opt/evm-cloud/docker-compose.yml
    permissions: '0644'
    content: |
      ${indent(6, docker_compose_content)}

%{ if enable_rpc_proxy && erpc_yaml_content != "" ~}
  # eRPC config
  - path: /opt/evm-cloud/config/erpc.yaml
    permissions: '0644'
    content: |
      ${indent(6, erpc_yaml_content)}
%{ endif ~}

%{ if enable_indexer && rindexer_yaml_content != "" ~}
  # rindexer config
  - path: /opt/evm-cloud/config/rindexer.yaml
    permissions: '0644'
    content: |
      ${indent(6, rindexer_yaml_content)}
%{ endif ~}

%{ for name, content in abi_files ~}
  # ABI: ${name}
  - path: /opt/evm-cloud/config/abis/${name}
    permissions: '0644'
    content: |
      ${indent(6, content)}
%{ endfor ~}
%{ endif ~}
