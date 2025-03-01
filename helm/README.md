# Iot Forest Helm Chart

This chart deploys the Iot Forest on a Kubernetes cluster using the Helm package manager.

```bash
helm install --create-namespace -n forest iot-forest ./forest

# To upgrade the deployment
helm upgrade -n forest iot-forest ./forest

# To delete the deployment
helm uninstall -n forest iot-forest
```

Port Forwarding Internal API

```bash
kubectl --namespace forest port-forward iot-forest-0 8080:8080
```

Exec Bash in the pod

```bash
kubectl exec -it -n forest iot-forest-0 -- /bin/bash
```

