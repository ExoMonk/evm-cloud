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
    deploy:
      resources:
        limits:
          memory: ${rpc_proxy_mem_limit}
    logging:
      driver: ${logging_driver}
      options:
%{ for key, value in logging_options ~}
        ${key}: ${value}
%{ endfor ~}
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
    command: ["start", "--path", "/config", "all"]
    env_file:
      - /opt/evm-cloud/.env
    deploy:
      resources:
        limits:
          memory: ${indexer_mem_limit}
%{ if enable_rpc_proxy ~}
    depends_on:
      erpc:
        condition: service_started
%{ endif ~}
    logging:
      driver: ${logging_driver}
      options:
%{ for key, value in logging_options ~}
        ${key}: ${value}
%{ endfor ~}
    networks:
      - evm-cloud
%{ endif ~}

networks:
  evm-cloud:
    driver: bridge
