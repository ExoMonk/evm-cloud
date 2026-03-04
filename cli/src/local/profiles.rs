pub(crate) struct ResourceSet {
    pub cpu_req: &'static str,
    pub mem_req: &'static str,
    pub cpu_lim: &'static str,
    pub mem_lim: &'static str,
}

pub(crate) struct ProfileResources {
    pub anvil: ResourceSet,
    pub erpc: ResourceSet,
    pub indexer: ResourceSet,
    pub clickhouse: ResourceSet,
}

pub(crate) fn resources(profile: super::Profile) -> ProfileResources {
    match profile {
        super::Profile::Default => ProfileResources {
            anvil: ResourceSet {
                cpu_req: "100m",
                mem_req: "128Mi",
                cpu_lim: "500m",
                mem_lim: "256Mi",
            },
            erpc: ResourceSet {
                cpu_req: "100m",
                mem_req: "128Mi",
                cpu_lim: "250m",
                mem_lim: "256Mi",
            },
            indexer: ResourceSet {
                cpu_req: "200m",
                mem_req: "256Mi",
                cpu_lim: "500m",
                mem_lim: "512Mi",
            },
            clickhouse: ResourceSet {
                cpu_req: "200m",
                mem_req: "512Mi",
                cpu_lim: "500m",
                mem_lim: "1Gi",
            },
        },
        super::Profile::Heavy => ProfileResources {
            anvil: ResourceSet {
                cpu_req: "500m",
                mem_req: "1Gi",
                cpu_lim: "2",
                mem_lim: "4Gi",
            },
            erpc: ResourceSet {
                cpu_req: "250m",
                mem_req: "256Mi",
                cpu_lim: "1",
                mem_lim: "512Mi",
            },
            indexer: ResourceSet {
                cpu_req: "500m",
                mem_req: "512Mi",
                cpu_lim: "2",
                mem_lim: "1Gi",
            },
            clickhouse: ResourceSet {
                cpu_req: "500m",
                mem_req: "1Gi",
                cpu_lim: "2",
                mem_lim: "2Gi",
            },
        },
    }
}
