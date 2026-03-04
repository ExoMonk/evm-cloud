use super::profiles::ResourceSet;

const CLICKHOUSE_YAML: &str = r#"apiVersion: v1
kind: Service
metadata:
  name: clickhouse
spec:
  type: NodePort
  ports:
    - name: http
      port: 8123
      targetPort: 8123
      nodePort: 30123
    - name: native
      port: 9000
      targetPort: 9000
  selector:
    app: clickhouse
---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: clickhouse
spec:
  serviceName: clickhouse
  replicas: 1
  selector:
    matchLabels:
      app: clickhouse
  template:
    metadata:
      labels:
        app: clickhouse
    spec:
      containers:
        - name: clickhouse
          image: clickhouse/clickhouse-server:24.3-alpine
          ports:
            - containerPort: 8123
            - containerPort: 9000
          env:
            - name: CLICKHOUSE_DB
              value: default
            - name: CLICKHOUSE_USER
              value: default
            - name: CLICKHOUSE_PASSWORD
              value: local-dev
          resources:
            requests:
              cpu: __CH_CPU_REQ__
              memory: __CH_MEM_REQ__
            limits:
              cpu: __CH_CPU_LIM__
              memory: __CH_MEM_LIM__
          readinessProbe:
            httpGet:
              path: /ping
              port: 8123
            initialDelaySeconds: 5
            periodSeconds: 5
          livenessProbe:
            httpGet:
              path: /ping
              port: 8123
            initialDelaySeconds: 10
            periodSeconds: 10
          volumeMounts:
            - name: data
              mountPath: /var/lib/clickhouse
            - name: config
              mountPath: /etc/clickhouse-server/config.d/local.xml
              subPath: local.xml
      volumes:
        - name: data
          __VOLUME_SOURCE__
        - name: config
          configMap:
            name: clickhouse-local-config
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: clickhouse-local-config
data:
  local.xml: |
    <clickhouse>
      <profiles>
        <default>
          <max_memory_usage>500000000</max_memory_usage>
        </default>
      </profiles>
    </clickhouse>
"#;

pub(crate) fn clickhouse_manifest(persist: bool, res: &ResourceSet) -> String {
    let volume_source = if persist {
        "hostPath:\n            path: /var/local-data/clickhouse\n            type: DirectoryOrCreate"
    } else {
        "emptyDir: {}"
    };

    CLICKHOUSE_YAML
        .replace("__CH_CPU_REQ__", res.cpu_req)
        .replace("__CH_MEM_REQ__", res.mem_req)
        .replace("__CH_CPU_LIM__", res.cpu_lim)
        .replace("__CH_MEM_LIM__", res.mem_lim)
        .replace("__VOLUME_SOURCE__", volume_source)
}
