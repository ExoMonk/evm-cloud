variable "kubeconfig_path" {
  description = "Path to kubeconfig for kind cluster."
  type        = string
  default     = ""
}

variable "kubeconfig_context" {
  description = "Kubeconfig context name for the kind cluster."
  type        = string
  default     = "kind-evm-cloud-test"
}

variable "erpc_config_yaml" {
  description = "eRPC config YAML for testing."
  type        = string
  default     = <<-YAML
    logLevel: info
    server:
      listenV4: true
      httpHostV4: 0.0.0.0
      httpPort: 4000
    projects:
      - id: main
        networks:
          - architecture: evm
            evm:
              chainId: 1
        upstreams:
          - id: public
            endpoint: https://eth.llamarpc.com
            type: evm
  YAML
}

variable "rindexer_config_yaml" {
  description = "rindexer config YAML for testing."
  type        = string
  default     = <<-YAML
    name: kind-test-indexer
    project_type: no-code
    networks:
      - name: ethereum
        chain_id: 1
        rpc: http://localhost:8545
    storage:
      clickhouse:
        enabled: true
    contracts: []
  YAML
}

variable "rindexer_abis" {
  description = "ABI map for testing."
  type        = map(string)
  default     = { "ERC20.json" = "{\"abi\": []}" }
}
