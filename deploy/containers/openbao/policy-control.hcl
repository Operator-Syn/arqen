path "secret/data/arqen/google/*" {
  capabilities = ["create", "read", "update"]
}

path "secret/metadata/arqen/google/*" {
  capabilities = ["read", "delete"]
}
