# SPDX-License-Identifier: MPL-2.0
FROM openbao/openbao:2.6.0

COPY --chmod=0644 deploy/containers/openbao/config.hcl /etc/openbao/config.hcl
COPY --chmod=0644 deploy/containers/openbao/policy-control.hcl /etc/arqen/policy-control.hcl
COPY --chmod=0644 deploy/containers/openbao/policy-broker.hcl /etc/arqen/policy-broker.hcl
COPY --chmod=0755 deploy/containers/openbao/bootstrap.sh /usr/local/bin/arqen-openbao-bootstrap.sh
COPY --chmod=0755 deploy/containers/openbao/start.sh /usr/local/bin/arqen-openbao-start.sh

USER 0:0
ENTRYPOINT ["/bin/sh", "/usr/local/bin/arqen-openbao-start.sh"]
