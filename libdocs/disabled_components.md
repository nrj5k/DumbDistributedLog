# Disabled Components Tracking

This file tracks components that have been temporarily disabled or not yet fully implemented in DDL.

## OpenRaft Integration

**Status**: In Progress - Minimal implementation complete

The cluster module has been reimplemented with a minimal Raft-compatible structure:

- `src/cluster/config.rs` - Cluster configuration with coordination port (6968)
- `src/cluster/node.rs` - RaftNode wrapper with transport
- `src/cluster/state_machine.rs` - Cluster state and commands

**Not Yet Implemented** (pending full OpenRaft integration):

1. **Full OpenRaft Integration**
    - Leader election using OpenRaft's Raft type config
    - Log replication for cluster commands
    - RaftNetworkFactory implementation for transport
    - Proper storage backend

2. **Coordination Protocol**
    - Raft RPC message serialization
    - AppendEntries/RequestVote handlers
    - InstallSnapshot support
    - Membership change protocol

## Data vs Coordination Separation

| Port | Purpose |
|------|---------|
| 6967 (data port) | Topic data, subscriptions |
| 6968 (coordination port) | Raft/cluster coordination |

## Next Steps

1. Complete OpenRaft type configuration with `declare_raft_types!`
2. Implement `RaftNetworkFactory` for transport
3. Add proper Raft storage backend
4. Wire up leader election flow
5. Test multi-node cluster formation
