# DumpLock Program

DumpLock is a neutral on-chain time-lock signaling protocol built on Solana.

It enables token creators to lock a fixed percentage of total token supply for a predefined time period to publicly signal commitment at launch.

---

## Program Information

- Network: Solana Mainnet
- Program ID: GoB3WxFTNYh8kqicpC1eYjnJCx9qpcy2TuGsVqB38pmz
- Website: https://dumplock.io
- Security Contact: security@dumplock.io

---

## Overview

DumpLock enforces strict and simple rules:

- Only one active lock per token
- Lock percentages: 95% / 97% / 99%
- Time options: 6h / 12h / 24h
- Lock percentage is always calculated from total supply
- Creator must hold the required percentage at lock time
- No partial unlock
- No auto-send
- Manual receive after unlock
- No staking
- No rewards
- No yield mechanics

DumpLock is not a DEX, staking platform, investment vehicle, or yield protocol.

It is a neutral time-based commitment signaling tool.

---

## Security

We welcome responsible disclosure of security vulnerabilities.

Please contact:

security@dumplock.io

Security policy:
https://dumplock.io/security

This program embeds on-chain security metadata using the solana-security-txt standard.

---

## Verifying the Deployed Program

To verify that the deployed program matches this source code:

### 1. Build the program locally

```
anchor build
```

### 2. Dump the deployed program from mainnet

```
solana program dump GoB3WxFTNYh8kqicpC1eYjnJCx9qpcy2TuGsVqB38pmz deployed.so
```

### 3. Compare SHA256 hashes

```
shasum -a 256 deployed.so
shasum -a 256 target/deploy/onchain_dumplock.so
```

If both hashes match, the deployed program corresponds exactly to this source code.

---

## License

MIT License
