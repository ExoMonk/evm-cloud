services:
%{ if enable_rpc_proxy ~}
  erpc:
    image: ${rpc_proxy_image}
    container_name: erpc
    restart: unless-stopped
    ports:
      - "4000:4000"
    volumes:
      - /opt/evm-cloud/config/erpc.yaml:/config/erpc.yaml:ro
    command: ["/erpc-server", "--config", "/config/erpc.yaml"]
    env_file:
      - /opt/evm-cloud/.env
    mem_limit: ${rpc_proxy_mem_limit}
    logging:
      driver: awslogs
      options:
        awslogs-region: ${aws_region}
        awslogs-group: ${log_group}
        awslogs-stream: erpc
    networks:
      - evm-cloud
%{ endif ~}

%{ if enable_indexer ~}
  rindexer:
    image: ${indexer_image}
    container_name: rindexer
    restart: unless-stopped
    volumes:
      - /opt/evm-cloud/config:/config:ro
    command: ["start", "--path", "/config", "indexer"]
    env_file:
      - /opt/evm-cloud/.env
%{ if storage_backend == "clickhouse" ~}
    environment:
      - CLICKHOUSE_URL=${clickhouse_url}
      - CLICKHOUSE_USER=${clickhouse_user}
      - CLICKHOUSE_DB=${clickhouse_db}
%{ endif ~}
    mem_limit: ${indexer_mem_limit}
%{ if enable_rpc_proxy ~}
    depends_on:
      erpc:
        condition: service_started
%{ endif ~}
    logging:
      driver: awslogs
      options:
        awslogs-region: ${aws_region}
        awslogs-group: ${log_group}
        awslogs-stream: rindexer
    networks:
      - evm-cloud
%{ endif ~}

networks:
  evm-cloud:
    driver: bridge
