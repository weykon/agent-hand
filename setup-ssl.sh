#!/bin/bash
# SSL setup script for auth.asymptai.com
# Run this after DNS is properly configured

echo "Checking DNS..."
IP=$(dig +short auth.asymptai.com @8.8.8.8 | head -1)
if [ "$IP" != "107.175.136.185" ]; then
    echo "ERROR: auth.asymptai.com points to $IP"
    echo "Expected: 107.175.136.185"
    echo "Please update your DNS A record first!"
    exit 1
fi

echo "DNS looks good! Obtaining SSL certificate..."
certbot --nginx -d auth.asymptai.com --non-interactive --agree-tos --email contact@asymptai.com

echo "Testing HTTPS..."
curl -s https://auth.asymptai.com/health

echo ""
echo "SSL certificate installed! Auto-renewal is configured via systemd timer."
