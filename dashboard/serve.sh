#!/bin/bash
cd "$(dirname "$0")"
IP=$(ipconfig getifaddr en0 2>/dev/null || ipconfig getifaddr en1 2>/dev/null)
echo "Dashboard: http://$IP:8080"
echo "Open that URL on the iPad"
python3 -m http.server 8080
