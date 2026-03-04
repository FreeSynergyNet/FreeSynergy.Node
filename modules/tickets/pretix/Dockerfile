# ==============================================================================
# FSN Platform – pretix with plugins
# ==============================================================================
# Build: podman build -t fsn-pretix .
# Update: podman pull pretix/standalone:stable && podman build -t fsn-pretix .
#
# Plugins are installed via pip. Add or remove as needed.
# After changing plugins, rebuild the image and redeploy.
# ==============================================================================

FROM pretix/standalone:stable

USER root

# Install all plugins in one layer
# - pretix-oidc:         OpenID Connect authentication (Kanidm integration)
# - pretix-pages:        Custom static pages for your shop
# - pretix-fontpack-free: Additional free fonts for PDF tickets
# - pretix-passbook:     Apple Wallet (.pkpass) ticket support
#
# Add more plugins here as pip packages, e.g.:
#   pretix-sepadebit     SEPA direct debit payment
#   pretix-mollie        Mollie payment provider
#   pretix-stripe        Stripe payment provider
#   pretix-slack         Slack notifications
RUN pip3 install --no-cache-dir \
    pretix-oidc \
    pretix-pages \
    pretix-fontpack-free \
    pretix-passbook \
    && rm -rf /root/.cache/pip

# Rebuild static files with plugins included
USER pretixuser
RUN cd /pretix/src && make production
