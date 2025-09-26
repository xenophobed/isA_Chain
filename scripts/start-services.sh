#!/bin/bash

# isA_Chain Backend Services Startup Script
# This script starts all microservices for the isA_Chain ecosystem

set -e

echo "
╔══════════════════════════════════════════════════════════╗
║          isA_Chain Microservices Launcher                ║
║                                                          ║
║  Starting all backend services...                       ║
╚══════════════════════════════════════════════════════════╝
"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Base directory - services are in the parent directory/services
BASE_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && cd ../services && pwd )"
PROJECT_ROOT="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && cd .. && pwd )"

# Services configuration
services_list="defi-api:8313 nft-api:8312 tools-api:8315"

# Blockchain configuration
HARDHAT_PORT=8545
HARDHAT_PID_FILE="$PROJECT_ROOT/.hardhat.pid"

# isA_Chain configuration
ISA_CHAIN_PORT=9944
ISA_CHAIN_P2P_PORT=9945
ISA_CHAIN_PID_FILE="$PROJECT_ROOT/.isachain.pid"

# Function to check if port is available
check_port() {
    local port=$1
    if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null ; then
        return 1
    else
        return 0
    fi
}

# Function to start Hardhat blockchain
start_hardhat() {
    echo -e "${YELLOW}Starting Hardhat test blockchain...${NC}"
    
    if [ -f "$HARDHAT_PID_FILE" ]; then
        local pid=$(cat "$HARDHAT_PID_FILE")
        if kill -0 $pid 2>/dev/null; then
            echo -e "${GREEN}✓ Hardhat already running (PID: $pid)${NC}"
            return 0
        fi
    fi
    
    if ! check_port $HARDHAT_PORT; then
        echo -e "${YELLOW}⚠ Port $HARDHAT_PORT is already in use${NC}"
        return 0
    fi
    
    cd "$PROJECT_ROOT"
    npx hardhat node > "$PROJECT_ROOT/hardhat.log" 2>&1 &
    local pid=$!
    echo $pid > "$HARDHAT_PID_FILE"
    
    echo -e "${YELLOW}  Waiting for Hardhat to start...${NC}"
    local count=0
    while [ $count -lt 30 ]; do
        if curl -s http://localhost:$HARDHAT_PORT > /dev/null 2>&1; then
            echo -e "${GREEN}✓ Hardhat started (PID: $pid)${NC}"
            return 0
        fi
        sleep 1
        count=$((count + 1))
    done
    
    echo -e "${RED}✗ Failed to start Hardhat${NC}"
    return 1
}

# Function to start isA_Chain
start_isachain() {
    echo -e "${YELLOW}Starting isA_Chain node...${NC}"
    
    if [ -f "$ISA_CHAIN_PID_FILE" ]; then
        local pid=$(cat "$ISA_CHAIN_PID_FILE")
        if kill -0 $pid 2>/dev/null; then
            echo -e "${GREEN}✓ isA_Chain already running (PID: $pid)${NC}"
            return 0
        fi
    fi
    
    if ! check_port $ISA_CHAIN_PORT; then
        echo -e "${YELLOW}⚠ Port $ISA_CHAIN_PORT is already in use${NC}"
        return 0
    fi
    
    # Check if binary exists
    if [ -f "$PROJECT_ROOT/target/debug/isa-chain-node" ]; then
        cd "$PROJECT_ROOT"
        ./target/debug/isa-chain-node > "$PROJECT_ROOT/isachain.log" 2>&1 &
        local pid=$!
        echo $pid > "$ISA_CHAIN_PID_FILE"
        
        echo -e "${GREEN}✓ isA_Chain started (PID: $pid)${NC}"
        echo -e "${GREEN}  RPC: http://localhost:$ISA_CHAIN_PORT${NC}"
        echo -e "${GREEN}  P2P: $ISA_CHAIN_P2P_PORT${NC}"
        return 0
    else
        echo -e "${RED}✗ isA_Chain binary not found. Please compile first:${NC}"
        echo -e "${YELLOW}  cd core/blockchain && cargo build --bin isa-chain-node${NC}"
        return 1
    fi
}

# Function to stop blockchains
stop_blockchain() {
    # Stop Hardhat
    if [ -f "$HARDHAT_PID_FILE" ]; then
        local pid=$(cat "$HARDHAT_PID_FILE")
        if kill -0 $pid 2>/dev/null; then
            echo -e "${YELLOW}  Stopping Hardhat (PID: $pid)...${NC}"
            kill $pid
            rm "$HARDHAT_PID_FILE"
            echo -e "${GREEN}  ✓ Hardhat stopped${NC}"
        fi
    fi
    
    # Stop isA_Chain
    if [ -f "$ISA_CHAIN_PID_FILE" ]; then
        local pid=$(cat "$ISA_CHAIN_PID_FILE")
        if kill -0 $pid 2>/dev/null; then
            echo -e "${YELLOW}  Stopping isA_Chain (PID: $pid)...${NC}"
            kill $pid
            rm "$ISA_CHAIN_PID_FILE"
            echo -e "${GREEN}  ✓ isA_Chain stopped${NC}"
        fi
    fi
}

# Function to start a service
start_service() {
    local service_name=$1
    local service_port=$2
    local service_dir="$BASE_DIR/$service_name"
    
    echo -e "${YELLOW}Starting $service_name on port $service_port...${NC}"
    
    # Check if service directory exists
    if [ ! -d "$service_dir" ]; then
        echo -e "${RED}✗ Service directory not found: $service_dir${NC}"
        return 1
    fi
    
    # Check if port is available
    if ! check_port $service_port; then
        echo -e "${RED}✗ Port $service_port is already in use${NC}"
        return 1
    fi
    
    # Install dependencies if needed
    if [ ! -d "$service_dir/node_modules" ]; then
        echo -e "${YELLOW}  Installing dependencies for $service_name...${NC}"
        cd "$service_dir" && npm install
    fi
    
    # Create .env file from example if it doesn't exist
    if [ ! -f "$service_dir/.env" ] && [ -f "$service_dir/.env.example" ]; then
        echo -e "${YELLOW}  Creating .env file from .env.example...${NC}"
        cp "$service_dir/.env.example" "$service_dir/.env"
    fi
    
    # Start the service in the background
    cd "$service_dir"
    npm run dev > "$service_dir/service.log" 2>&1 &
    local pid=$!
    
    # Store PID for later
    echo $pid > "$service_dir/.pid"
    
    # Wait a bit to check if service started successfully
    sleep 3
    
    if kill -0 $pid 2>/dev/null; then
        echo -e "${GREEN}✓ $service_name started successfully (PID: $pid)${NC}"
        return 0
    else
        echo -e "${RED}✗ Failed to start $service_name${NC}"
        return 1
    fi
}

# Function to stop all services
stop_all_services() {
    echo -e "\n${YELLOW}Stopping all services...${NC}"
    
    # Stop microservices
    for service_entry in $services_list; do
        IFS=':' read -r service_name service_port <<< "$service_entry"
        local service_dir="$BASE_DIR/$service_name"
        local pid_file="$service_dir/.pid"
        
        if [ -f "$pid_file" ]; then
            local pid=$(cat "$pid_file")
            if kill -0 $pid 2>/dev/null; then
                echo -e "${YELLOW}  Stopping $service_name (PID: $pid)...${NC}"
                kill $pid
                rm "$pid_file"
                echo -e "${GREEN}  ✓ $service_name stopped${NC}"
            fi
        fi
    done
    
    # Stop blockchain if requested
    if [ "${STOP_BLOCKCHAIN:-false}" = "true" ]; then
        stop_blockchain
    fi
}

# Function to show service status
show_status() {
    echo -e "\n${YELLOW}Service Status:${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    # Show Hardhat blockchain status
    if [ -f "$HARDHAT_PID_FILE" ]; then
        local hardhat_pid=$(cat "$HARDHAT_PID_FILE")
        if kill -0 $hardhat_pid 2>/dev/null; then
            echo -e "${GREEN}✓${NC} Hardhat (Port: $HARDHAT_PORT) - Running (PID: $hardhat_pid)"
        else
            echo -e "${RED}✗${NC} Hardhat (Port: $HARDHAT_PORT) - Not running"
        fi
    else
        if ! check_port $HARDHAT_PORT; then
            echo -e "${GREEN}✓${NC} Hardhat (Port: $HARDHAT_PORT) - Running (external)"
        else
            echo -e "${RED}✗${NC} Hardhat (Port: $HARDHAT_PORT) - Not running"
        fi
    fi
    
    # Show isA_Chain blockchain status
    if [ -f "$ISA_CHAIN_PID_FILE" ]; then
        local isachain_pid=$(cat "$ISA_CHAIN_PID_FILE")
        if kill -0 $isachain_pid 2>/dev/null; then
            echo -e "${GREEN}✓${NC} isA_Chain (Port: $ISA_CHAIN_PORT) - Running (PID: $isachain_pid)"
        else
            echo -e "${RED}✗${NC} isA_Chain (Port: $ISA_CHAIN_PORT) - Not running"
        fi
    else
        if ! check_port $ISA_CHAIN_PORT; then
            echo -e "${GREEN}✓${NC} isA_Chain (Port: $ISA_CHAIN_PORT) - Running (external)"
        else
            echo -e "${RED}✗${NC} isA_Chain (Port: $ISA_CHAIN_PORT) - Not running"
        fi
    fi
    
    # Show Consul status
    if curl -s http://localhost:8500/v1/agent/self > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} Consul (Port: 8500) - Running"
    else
        echo -e "${YELLOW}⚠${NC} Consul (Port: 8500) - Not running"
    fi
    
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    # Show microservices status
    for service_entry in $services_list; do
        IFS=':' read -r service_name service_port <<< "$service_entry"
        local service_dir="$BASE_DIR/$service_name"
        local pid_file="$service_dir/.pid"
        
        if [ -f "$pid_file" ]; then
            local pid=$(cat "$pid_file")
            if kill -0 $pid 2>/dev/null; then
                echo -e "${GREEN}✓${NC} $service_name (Port: $service_port) - Running (PID: $pid)"
            else
                echo -e "${RED}✗${NC} $service_name (Port: $service_port) - Not running"
            fi
        else
            echo -e "${RED}✗${NC} $service_name (Port: $service_port) - Not running"
        fi
    done
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
}

# Function to tail logs
tail_logs() {
    echo -e "\n${YELLOW}Tailing service logs (Ctrl+C to stop)...${NC}"
    
    # Create a command to tail all service logs
    tail_cmd="tail -f"
    for service_entry in $services_list; do
        IFS=':' read -r service_name service_port <<< "$service_entry"
        local service_dir="$BASE_DIR/$service_name"
        if [ -f "$service_dir/service.log" ]; then
            tail_cmd="$tail_cmd $service_dir/service.log"
        fi
    done
    
    eval $tail_cmd
}

# Handle script termination (only for stop and restart commands)
# trap stop_all_services EXIT

# Main execution
case "${1:-start}" in
    start)
        # Check prerequisites
        echo -e "${YELLOW}Checking prerequisites...${NC}"
        
        # Check if Node.js is installed
        if ! command -v node &> /dev/null; then
            echo -e "${RED}✗ Node.js is not installed${NC}"
            exit 1
        fi
        
        # Check if npm is installed
        if ! command -v npm &> /dev/null; then
            echo -e "${RED}✗ npm is not installed${NC}"
            exit 1
        fi
        
        # Check if Hardhat is installed
        if ! npm list hardhat > /dev/null 2>&1; then
            echo -e "${YELLOW}⚠ Hardhat not found locally, will use npx${NC}"
        fi
        
        # Check if Consul is running (optional but recommended)
        if ! curl -s http://localhost:8500/v1/agent/self > /dev/null 2>&1; then
            echo -e "${YELLOW}⚠ Warning: Consul is not running on localhost:8500${NC}"
            echo -e "${YELLOW}  Services will start but won't register with Consul${NC}"
        else
            echo -e "${GREEN}✓ Consul is running${NC}"
        fi
        
        echo ""
        
        # Ask user which blockchain to use
        echo -e "${YELLOW}Choose blockchain:${NC}"
        echo "  1) Hardhat (test/development)"
        echo "  2) isA_Chain (your custom blockchain)"
        echo "  3) Both"
        read -p "Enter choice [1-3] (default: 1): " blockchain_choice
        blockchain_choice=${blockchain_choice:-1}
        
        case $blockchain_choice in
            1)
                start_hardhat
                ;;
            2) 
                start_isachain
                ;;
            3)
                start_hardhat
                start_isachain
                ;;
            *)
                echo -e "${RED}Invalid choice. Using Hardhat.${NC}"
                start_hardhat
                ;;
        esac
        
        echo ""
        
        # Start all services
        failed_services=()
        for service_entry in $services_list; do
            IFS=':' read -r service_name service_port <<< "$service_entry"
            if ! start_service "$service_name" "$service_port"; then
                failed_services+=($service_name)
            fi
            echo ""
        done
        
        # Show final status
        show_status
        
        # Show access URLs
        echo -e "\n${GREEN}Services are ready!${NC}"
        echo -e "\nAccess services via Gateway:"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        echo "DeFi Service:  http://localhost:8000/api/v1/defi-service/"
        echo "NFT Service:   http://localhost:8000/api/v1/nft-service/"
        echo "Tools Service: http://localhost:8000/api/v1/tools-service/"
        echo ""
        echo "Direct access (for development):"
        echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        for service_entry in $services_list; do
            IFS=':' read -r service_name service_port <<< "$service_entry"
            echo "$service_name: http://localhost:${service_port}/health"
        done
        echo ""
        
        # Check if any services failed
        if [ ${#failed_services[@]} -gt 0 ]; then
            echo -e "${RED}⚠ The following services failed to start:${NC}"
            for service in "${failed_services[@]}"; do
                echo -e "${RED}  - $service${NC}"
            done
            echo -e "${YELLOW}Check the logs in the service directory for more information${NC}"
            exit 1
        fi
        
        echo -e "${GREEN}All services started successfully!${NC}"
        echo -e "${YELLOW}Use '$0 logs' to tail service logs${NC}"
        echo -e "${YELLOW}Use '$0 stop' to stop all services${NC}"
        echo -e "${YELLOW}Use '$0 status' to check service status${NC}"
        ;;
        
    stop)
        stop_all_services
        ;;
    
    stop-all)
        STOP_BLOCKCHAIN=true stop_all_services
        ;;
        
    restart)
        stop_all_services
        sleep 2
        $0 start
        ;;
        
    restart-all)
        STOP_BLOCKCHAIN=true stop_all_services
        sleep 2
        $0 start
        ;;
        
    status)
        show_status
        ;;
        
    logs)
        tail_logs
        ;;
    
    blockchain)
        case "${2:-status}" in
            start)
                start_blockchain
                ;;
            stop)
                stop_blockchain
                ;;
            status)
                echo -e "${YELLOW}Blockchain Status:${NC}"
                
                # Hardhat status
                if [ -f "$HARDHAT_PID_FILE" ]; then
                    pid=$(cat "$HARDHAT_PID_FILE")
                    if kill -0 $pid 2>/dev/null; then
                        echo -e "${GREEN}✓ Hardhat running (PID: $pid)${NC}"
                        echo -e "  RPC URL: http://localhost:$HARDHAT_PORT"
                    else
                        echo -e "${RED}✗ Hardhat not running${NC}"
                    fi
                else
                    if ! check_port $HARDHAT_PORT; then
                        echo -e "${GREEN}✓ Hardhat running (external process)${NC}"
                        echo -e "  RPC URL: http://localhost:$HARDHAT_PORT"
                    else
                        echo -e "${RED}✗ Hardhat not running${NC}"
                    fi
                fi
                
                # isA_Chain status
                if [ -f "$ISA_CHAIN_PID_FILE" ]; then
                    pid=$(cat "$ISA_CHAIN_PID_FILE")
                    if kill -0 $pid 2>/dev/null; then
                        echo -e "${GREEN}✓ isA_Chain running (PID: $pid)${NC}"
                        echo -e "  RPC URL: http://localhost:$ISA_CHAIN_PORT"
                    else
                        echo -e "${RED}✗ isA_Chain not running${NC}"
                    fi
                else
                    if ! check_port $ISA_CHAIN_PORT; then
                        echo -e "${GREEN}✓ isA_Chain running (external process)${NC}"
                        echo -e "  RPC URL: http://localhost:$ISA_CHAIN_PORT"
                    else
                        echo -e "${RED}✗ isA_Chain not running${NC}"
                    fi
                fi
                ;;
            logs)
                echo -e "${YELLOW}Available blockchain logs:${NC}"
                if [ -f "$PROJECT_ROOT/hardhat.log" ]; then
                    echo -e "${GREEN}Hardhat logs: $PROJECT_ROOT/hardhat.log${NC}"
                fi
                if [ -f "$PROJECT_ROOT/isachain.log" ]; then
                    echo -e "${GREEN}isA_Chain logs: $PROJECT_ROOT/isachain.log${NC}"
                fi
                
                if [ -f "$PROJECT_ROOT/hardhat.log" ] && [ -f "$PROJECT_ROOT/isachain.log" ]; then
                    echo -e "\n${YELLOW}Tailing both blockchain logs:${NC}"
                    tail -f "$PROJECT_ROOT/hardhat.log" "$PROJECT_ROOT/isachain.log"
                elif [ -f "$PROJECT_ROOT/hardhat.log" ]; then
                    echo -e "\n${YELLOW}Tailing Hardhat logs:${NC}"
                    tail -f "$PROJECT_ROOT/hardhat.log"
                elif [ -f "$PROJECT_ROOT/isachain.log" ]; then
                    echo -e "\n${YELLOW}Tailing isA_Chain logs:${NC}"
                    tail -f "$PROJECT_ROOT/isachain.log"
                else
                    echo -e "${RED}No blockchain logs found${NC}"
                fi
                ;;
            *)
                echo "Usage: $0 blockchain {start|stop|status|logs}"
                exit 1
                ;;
        esac
        ;;
        
    *)
        echo "Usage: $0 {start|stop|stop-all|restart|restart-all|status|logs|blockchain}"
        echo ""
        echo "Commands:"
        echo "  start       - Start all services (blockchain if needed + microservices)"
        echo "  stop        - Stop microservices only"
        echo "  stop-all    - Stop all services including blockchain"
        echo "  restart     - Restart microservices only"
        echo "  restart-all - Restart everything including blockchain"
        echo "  status      - Show status of all services"
        echo "  logs        - Tail logs of all microservices"
        echo "  blockchain  - Manage blockchain separately"
        echo ""
        echo "Blockchain subcommands:"
        echo "  $0 blockchain start  - Start blockchain node"
        echo "  $0 blockchain stop   - Stop blockchain node"
        echo "  $0 blockchain status - Show blockchain status"
        echo "  $0 blockchain logs   - Tail blockchain logs"
        exit 1
        ;;
esac