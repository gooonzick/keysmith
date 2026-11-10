#!/usr/bin/env bash
# Makes a throwaway home with a sample ~/.ssh for screenshots and trying Keysmith out:
#   scripts/demo-home.sh /tmp/ks-demo && KEYSMITH_HOME=/tmp/ks-demo cargo run
set -euo pipefail
HOME_DIR="${1:?usage: demo-home.sh DIR}"
rm -rf "$HOME_DIR"
mkdir -p "$HOME_DIR/.ssh/config.d"
chmod 700 "$HOME_DIR/.ssh"
cd "$HOME_DIR/.ssh"
ssh-keygen -q -t ed25519 -N "" -C "george@laptop" -f id_ed25519
ssh-keygen -q -t ed25519 -N "demo pass" -C "work laptop" -f id_ed25519_work
ssh-keygen -q -t rsa -b 3072 -N "" -C "legacy" -f id_rsa_legacy
chmod 644 id_rsa_legacy
cat > config <<'CFG'
# Personal machines and work boxes
Include config.d/*.conf
AddKeysToAgent yes

# GitHub, personal account
Host github.com
  HostName github.com
  User git
  IdentityFile ~/.ssh/id_ed25519
  IdentitiesOnly yes

# Production API, behind the bastion
Host prod-api
  HostName 10.20.1.15
  User deploy
  Port 2222
  ProxyJump bastion
  IdentityFile ~/.ssh/id_ed25519_work

Host bastion
  HostName bastion.example.com
  User george
  IdentityFile ~/.ssh/id_ed25519_work
  ForwardAgent no

# Old NAS at home
Host nas
  HostName 192.168.1.20
  User admin
  IdentityFile ~/.ssh/id_rsa_legacy
  HostKeyAlgorithms +ssh-rsa

Host *
  ServerAliveInterval 30
  ServerAliveCountMax 3
CFG
chmod 600 config
cat > config.d/lab.conf <<'CFG'
# Raspberry Pi in the lab
Host pi
  HostName 192.168.1.42
  User pi
  IdentityFile ~/.ssh/id_missing
CFG
echo "Demo home ready: KEYSMITH_HOME=$HOME_DIR"
