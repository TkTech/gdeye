extends Node

var _peer: SteamMultiplayerPeer

# This should NOT be flagged: SteamMultiplayerPeer extends MultiplayerPeer
func get_multiplayer_peer() -> MultiplayerPeer:
    return _peer

# This SHOULD be flagged: String is not compatible with int
func get_wrong_type() -> int:
    return "hello"
