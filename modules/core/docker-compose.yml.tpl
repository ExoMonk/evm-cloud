services:
%{ if enable_rpc_proxy ~}
  erpc:
    image: ${rpc_proxy_image}
    container_name: erpc
    restart: unless-stopped
%{ if enable_caddy ~}
    expose:
      - "4000"
%{ else ~}
    ports:
      - "4000:4000"
%{ endif ~}
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

%{ if enable_caddy ~}
  caddy:
    image: ${caddy_image}
    container_name: caddy
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /opt/evm-cloud/config/Caddyfile:/etc/caddy/Caddyfile:ro
%{ if caddy_cert_volumes != "" ~}
      ${caddy_cert_volumes}
%{ endif ~}
      - caddy_data:/data
      - caddy_config:/config
    deploy:
      resources:
        limits:
          memory: ${caddy_mem_limit}
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
%{ if enable_caddy ~}

volumes:
  caddy_data:
  caddy_config:
%{ endif ~}
