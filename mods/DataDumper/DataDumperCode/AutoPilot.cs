using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.AutoSlay.Helpers;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Context;
using MegaCrit.Sts2.Core.DevConsole;
using MegaCrit.Sts2.Core.DevConsole.ConsoleCommands;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Saves;
using MegaCrit.Sts2.Core.Settings;
using MegaCrit.Sts2.Core.Entities.CardRewardAlternatives;
using MegaCrit.Sts2.Core.TestSupport;

namespace DataDumper.DataDumperCode;

/// <summary>
/// Headless card-selection answerer for in-combat prompts (Armaments' upgrade-pick, Burning Pact /
/// Brand's exhaust-pick, Headbutt's discard-pick, …). The game's <see cref="CardSelectCmd"/> consults a
/// pushed <see cref="ICardSelector"/> (its own TestSupport hook) *instead of* showing a UI screen — so
/// answering at this layer never touches Godot's scene tree and avoids the threading race that sank the
/// earlier UI-driving attempt. Picks are deterministic (first N that satisfy the prompt's min/max); the
/// trace records the resulting plays, so the *choice* is captured downstream regardless of which we pick.
/// </summary>
public sealed class AutoPilotCardSelector : ICardSelector
{
    public Task<IEnumerable<CardModel>> GetSelectedCards(IEnumerable<CardModel> options, int minSelect, int maxSelect)
    {
        var list = options.ToList();
        if (list.Count == 0) return Task.FromResult<IEnumerable<CardModel>>(System.Array.Empty<CardModel>());
        // Take as many as the prompt allows but at least the required minimum (clamped to availability) —
        // mirrors AutoSlayCardSelector, but deterministic (first N) for reproducible traces.
        int take = System.Math.Min(maxSelect, list.Count);
        if (take < minSelect) take = System.Math.Min(minSelect, list.Count);
        return Task.FromResult<IEnumerable<CardModel>>(list.Take(take).ToList());
    }

    // Card-reward selection is a post-combat screen never reached by the combat autopilot; implement
    // minimally to satisfy the interface (pick the first option, no alternative).
    public CardRewardSelection GetSelectedCardReward(
        System.Collections.Generic.IReadOnlyList<MegaCrit.Sts2.Core.Entities.Cards.CardCreationResult> options,
        System.Collections.Generic.IReadOnlyList<CardRewardAlternative> alternatives)
        => new CardRewardSelection { card = options.Count > 0 ? options[0].Card : null, alternative = null };
}

/// <summary>
/// Console command `autopilot`: auto-plays the CURRENT combat via the genuine manual-play path
/// (so the CombatOracle records it as real player actions) with NO god-mode buffs, so the resulting
/// trace reflects true combat resolution for differential validation.
///
/// Usage (in the dev console, opened with `):
///   fight CULTISTS_NORMAL
///   kill 1            (optional: drop the second monster)
///   autopilot         (plays the fight to completion; trace lands in data/combat_traces/)
///
/// The game registers this automatically via ReflectionHelper.GetSubtypesInMods&lt;AbstractConsoleCmd&gt;().
/// </summary>
public sealed class AutoPilotConsoleCmd : AbstractConsoleCmd
{
    public override string CmdName => "autopilot";
    public override string Args => "";
    public override string Description => "Auto-plays the current combat (real manual plays, no buffs) so the CombatOracle records a trace.";
    public override bool IsNetworked => false;

    public override CmdResult Process(Player? issuingPlayer, string[] args)
    {
        AutoPilot.Start();
        return new CmdResult(success: true, "Autopilot engaged — will play the current/next combat to completion.");
    }
}

public static class AutoPilot
{
    public static void Start()
    {
        // Fire-and-forget on the game loop; exceptions are caught inside.
        _ = RunAsync();
    }

    private static async Task RunAsync()
    {
        using var cts = new CancellationTokenSource(System.TimeSpan.FromMinutes(3));
        FastModeType prevFast = default;
        bool fastSet = false;
        try { prevFast = SaveManager.Instance.PrefsSave.FastMode; SaveManager.Instance.PrefsSave.FastMode = FastModeType.Instant; fastSet = true; }
        catch { /* speed tweak is best-effort */ }

        try { await DriveCombatAsync(cts.Token); }
        catch (System.Exception e) { MainFile.Logger.Info($"Autopilot error: {e.Message}", 0); }
        finally { if (fastSet) try { SaveManager.Instance.PrefsSave.FastMode = prevFast; } catch { } }
    }

    /// <summary>
    /// Plays the current combat to completion via genuine manual plays (isAutoPlay=false), no buffs.
    /// Reused by the console command and the headless batch driver. Returns when combat ends.
    /// </summary>
    public static async Task DriveCombatAsync(CancellationToken ct)
    {
        await WaitHelper.Until(() => CombatManager.Instance.IsInProgress, ct, System.TimeSpan.FromSeconds(20), "combat to start");
        MainFile.Logger.Info("Autopilot: starting.", 0);

        // Answer any in-combat selection prompts (Armaments / Burning Pact / Brand / Headbutt) headlessly
        // via the model-layer selector hook, so they don't block waiting on a UI screen we can't click.
        using var _selector = CardSelectCmd.PushSelector(new AutoPilotCardSelector());

        int turn = 0;
        while (CombatManager.Instance.IsInProgress && turn < 100)
        {
            turn++;
            // Always read the player from the live COMBAT state — the run-manager's player can be a
            // distinct/stale object whose per-turn energy/hand never advances.
            var st = CombatManager.Instance.DebugOnlyGetState();
            var player = st?.Players?.FirstOrDefault();
            if (player == null) break;
            int alive = st!.Enemies.Count(e => e.IsAlive);
            MainFile.Logger.Info($"Autopilot: turn {turn} begin (enemiesAlive={alive})", 0);
            // Ready to act = our play phase AND the engine has enabled player actions. Gating on
            // PlayerActionsDisabled avoids acting on the previous (ending) turn's stale state.
            await WaitHelper.Until(
                () => (player.PlayerCombatState?.Phase == PlayerTurnPhase.Play && !CombatManager.Instance.PlayerActionsDisabled)
                      || !CombatManager.Instance.IsInProgress,
                ct, System.TimeSpan.FromSeconds(30), "play phase ready");
            if (!CombatManager.Instance.IsInProgress) break;

            var attempted = new System.Collections.Generic.HashSet<CardModel>();
            int played = 0;
            while (played < 50)
            {
                var pcs = player.PlayerCombatState;
                if (pcs == null || pcs.Phase != PlayerTurnPhase.Play || CombatManager.Instance.PlayerActionsDisabled) break;

                var hand = PileType.Hand.GetPile(player);
                // Affordability gate: CanPlay() does NOT check energy, so without this the loop would
                // force-play an expensive card after cheaper ones have drained energy (e.g. a 3-cost
                // attack with 1 energy left). That produces an over-spent turn the energy-faithful solver
                // rejects ("Not enough energy"). EnergyCost.GetAmountToSpend() already reflects X-cost and
                // any reductions, so `cost <= energy` is the correct, faithful gate.
                int CostOf(CardModel c) { try { return c.EnergyCost.GetAmountToSpend(); } catch { return 0; } }
                int energyNow = pcs.Energy;
                var playable = hand.Cards
                    .Where(c => !attempted.Contains(c)
                                && c.CanPlay(out UnplayableReason _, out AbstractModel _)
                                && CostOf(c) <= energyNow)
                    .ToList();
                if (playable.Count == 0)
                {
                    if (hand.Cards.Count > 0 && played == 0)
                    {
                        var c0 = hand.Cards[0];
                        c0.CanPlay(out UnplayableReason why, out AbstractModel _);
                        MainFile.Logger.Info($"Autopilot: no playable cards. hand={hand.Cards.Count} energy={player.PlayerCombatState?.Energy} first={c0.GetType().Name} reason={why} actionsDisabled={CombatManager.Instance.PlayerActionsDisabled}", 0);
                    }
                    break;
                }

                // Play order: Powers first (setup like Demon Form / Inflame — otherwise cheap attacks eat
                // all the energy and an expensive Power never gets played), then Skills (block/Vuln/
                // exhaust setup), then Attacks last. Setup-before-attacks is good play AND it actually
                // exercises combo/conditional cards live — Body Slam reads built-up Block, Bully reads
                // applied Vulnerable, Rage's block-per-attack triggers on the trailing attacks, etc.
                // Within a tier, prefer the most expensive affordable card so big cards (Mangle, Bludgeon)
                // aren't starved when cheap cards would otherwise eat the energy first.
                var card = playable
                    .OrderByDescending(c => c.Type == CardType.Power ? 2 : c.Type == CardType.Skill ? 1 : 0)
                    .ThenByDescending(CostOf)
                    .First();
                var target = PickTarget(card);
                attempted.Add(card);

                if (card.TryManualPlay(target))
                {
                    await WaitHelper.Until(
                        () => !hand.Cards.Contains(card) || !CombatManager.Instance.IsInProgress,
                        ct, System.TimeSpan.FromSeconds(15), "card resolve");
                    played++;
                }
            }

            MainFile.Logger.Info($"Autopilot: turn {turn} played {played} card(s), ending turn", 0);
            var pcs2 = player.PlayerCombatState;
            if (pcs2 != null && pcs2.Phase == PlayerTurnPhase.Play && CombatManager.Instance.IsInProgress)
                PlayerCmd.EndTurn(player, canBackOut: false);

            // Confirm the turn actually ended (actions disabled / phase changed) before looping back,
            // so the next iteration's "play phase ready" wait doesn't trip on this turn's stale state.
            await WaitHelper.Until(
                () => CombatManager.Instance.PlayerActionsDisabled
                      || player.PlayerCombatState?.Phase != PlayerTurnPhase.Play
                      || !CombatManager.Instance.IsInProgress,
                ct, System.TimeSpan.FromSeconds(30), "turn ending");
        }
        MainFile.Logger.Info($"Autopilot: combat finished (inProgress={CombatManager.Instance.IsInProgress}, turns={turn}).", 0);
    }

    private static Creature? PickTarget(CardModel card)
    {
        if (card.TargetType != TargetType.AnyEnemy) return null;
        var state = CombatManager.Instance.DebugOnlyGetState();
        return state?.Enemies.Where(e => e.IsAlive).OrderBy(e => e.CurrentHp).FirstOrDefault();
    }
}
