#!/bin/bash
# Wait for DNS to propagate then install SSL

echo "Waiting for DNS to propagate..."

while true; do
    IP=$(dig +short auth.asymptai.com @8.8.8.8 | head -1)
    if [ "$IP" = "107.175.136.185" ]; then
        echo "[$(date)] DNS is ready: $IP"
        break
    fi
    echo "[$(date)] Current DNS: $IP, waiting for 107.175.136.185..."
    sleep 30
done

echo "Installing SSL certificate..."
if certbot --nginx -d auth.asymptai.com --non-interactive --agree-tos --email contact@asymptai.com; then
    echo "[$(date)] SSL installed successfully!"
    echo "Testing HTTPS..."
    curl -s https://auth.asymptai.com/health
    echo ""
    echo "All done! https://auth.asymptai.com is ready"
else
    echo "[$(date)] SSL installation failed, check /var/log/letsencrypt/letsencrypt.log"
fi
