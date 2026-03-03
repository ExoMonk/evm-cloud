output "worker_nodes" {
  description = "Resolved worker node details with cluster node names."
  value = [for node in var.worker_nodes : {
    name = "${var.project_name}-worker-${node.name}"
    host = node.host
    role = node.role
  }]
}
