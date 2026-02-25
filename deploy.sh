#!/bin/bash
# Deploy script for agent-hand auth service
# Usage: ./deploy.sh [frontend|backend|all]

set -e

SERVER="usa"
REMOTE_DIR="/root/asympt-auth"
FRONTEND_DIR="docs"

colors() {
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    RED='\033[0;31m'
    NC='\033[0m'
}

colors

log() {
    echo -e "${GREEN}[DEPLOY]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

deploy_frontend() {
    log "Deploying frontend to GitHub Pages..."
    git add docs/
    git commit -m "chore: update frontend" || warn "No changes to commit"
    git push origin master
    log "Frontend pushed. GitHub Pages will update shortly."
    log "Check status at: https://weykon.github.io/agent-hand"
}

deploy_backend() {
    log "Deploying backend to $SERVER..."

    # Check server connectivity
    if ! ssh -o ConnectTimeout=5 $SERVER "echo 'OK'" > /dev/null 2>&1; then
        error "Cannot connect to server $SERVER"
    fi

    # Sync auth service code
    log "Uploading auth service code..."
    rsync -avz --delete \
        --exclude 'node_modules' \
        --exclude 'dist' \
        agent-hand-auth/ $SERVER:$REMOTE_DIR/app/

    # Restart service
    log "Restarting auth service..."
    ssh $SERVER "cd $REMOTE_DIR && docker-compose restart auth"

    # Health check with retry
    log "Running health check..."
    for i in 1 2 3; do
        sleep 3
        if ssh $SERVER "curl -sf http://127.0.0.1:3100/health > /dev/null 2>&1"; then
            log "✓ Auth service is healthy"
            break
        fi
        if [ $i -eq 3 ]; then
            error "Auth service health check failed after 3 retries"
        fi
        warn "Retry $i/3..."
    done

    log "Backend deployed successfully!"
}

deploy_all() {
    deploy_backend
    deploy_frontend
}

show_help() {
    cat << EOF
Usage: ./deploy.sh [COMMAND]

Commands:
    frontend    Deploy frontend (docs/) to GitHub Pages
    backend     Deploy backend (auth service) to server
    all         Deploy both frontend and backend (default)
    help        Show this help message

Examples:
    ./deploy.sh              # Deploy everything
    ./deploy.sh backend      # Deploy only backend
    ./deploy.sh frontend     # Deploy only frontend
EOF
}

main() {
    case "${1:-all}" in
        frontend)
            deploy_frontend
            ;;
        backend)
            deploy_backend
            ;;
        all)
            deploy_all
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            error "Unknown command: $1. Run './deploy.sh help' for usage."
            ;;
    esac
}

main "$@"
