#!/bin/bash
set -euo pipefail

NETWORK=${1:-testnet}
SOURCE_ACCOUNT=${2}
WASM_TARGET=${WASM_TARGET:-wasm32v1-none}
WASM_ARTIFACT=${WASM_ARTIFACT:-target/${WASM_TARGET}/release/craft_nexus_contract.wasm}

if [ -z "$SOURCE_ACCOUNT" ]; then
    echo "Usage: ./scripts/deploy.sh [testnet|mainnet] <SOURCE_ACCOUNT>"
    echo "Example: ./scripts/deploy.sh testnet alice"
    exit 1
fi

if [ ! -f "$WASM_ARTIFACT" ]; then
    echo "WASM artifact not found at ${WASM_ARTIFACT}. Running build first..."
    ./scripts/build.sh
fi

echo "Deploying to $NETWORK..."

# Set network configuration
if [ "$NETWORK" = "testnet" ]; then
    RPC_URL="https://soroban-testnet.stellar.org:443"
    NETWORK_PASSPHRASE="Test SDF Network ; September 2015"
elif [ "$NETWORK" = "mainnet" ]; then
    RPC_URL="https://soroban-rpc.mainnet.stellar.org:443"
    NETWORK_PASSPHRASE="Public Global Stellar Network ; September 2015"
else
    echo "Invalid network. Use 'testnet' or 'mainnet'"
    exit 1
fi

# Configure network alias for future commands
stellar network add \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    "$NETWORK" 2>/dev/null || true

# Deploy
echo "Deploying contract..."
CONTRACT_ID=$(stellar contract deploy \
    --wasm "$WASM_ARTIFACT" \
    --source-account "$SOURCE_ACCOUNT" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    --network "$NETWORK")

echo ""
echo "Contract deployed successfully!"
echo "Contract ID: $CONTRACT_ID"
echo ""
echo "Add this to your .env.local:"
echo "NEXT_PUBLIC_ESCROW_CONTRACT_ADDRESS=$CONTRACT_ID"
