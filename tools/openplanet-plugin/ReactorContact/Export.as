// Reactor Contact -- exports for other plugins (tm_Reactor-Duration).
// All read-only. Every function fails closed: when the car object cannot be
// resolved and validated for `player`, the booleans are false and the
// integers 0, and Status() says why.
namespace ReactorContact
{
	// True on a frame where the player's car touched a reactor-granting
	// surface (pad, ring or gate; gameplay materials ReactorBoost, ReactorBoost2
	// and their Oriented gate variants) on the current physics tick.
	import bool IsTouching(CSmPlayer@ player) from "ReactorContact";

	// The playground GameTime (ms) of the last physics tick in reactor contact,
	// 0 if the car never touched one since it was created. Refreshed every
	// tick while touching; frozen otherwise (it is NOT a countdown).
	import uint LastContactTime(CSmPlayer@ player) from "ReactorContact";

	// GameTime (ms) of the tick the current/last boost was activated (the
	// first contact while no boost was active).
	import uint ActivationTime(CSmPlayer@ player) from "ReactorContact";

	// The boost duration the game armed at the last contact, in ms (6000 for
	// the stock tuning); the boost ends at LastContactTime + Duration.
	import uint Duration(CSmPlayer@ player) from "ReactorContact";

	// 0 none, 1 = ReactorBoost (yellow), 2 = ReactorBoost2 (red). Zero once the
	// boost has expired (the same value CSceneVehicleVisState.ReactorBoostLvl carries).
	import uint Level(CSmPlayer@ player) from "ReactorContact";

	// 0 none, 1 up, 2 down (CSceneVehicleVisState.ReactorBoostType).
	import uint Type(CSmPlayer@ player) from "ReactorContact";

	// Whether the car object behind `player` was found and passed every check.
	import bool IsResolved(CSmPlayer@ player) from "ReactorContact";

	// Human-readable resolution status for the local player.
	import string Status() from "ReactorContact";
}
