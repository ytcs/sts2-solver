using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>
/// A once-per-combat characterisation of the player's deck used by the sound horizon/loss bounds: how much
/// block (and, for the damage-budget bounds, how much damage) it can produce per deck-cycle, whether it can
/// apply Weak, and whether its damage is state-independent enough to upper-bound. Built by playing one copy
/// of each card on a controlled probe (the real engine), so it stays faithful without re-implementing card
/// effects. Returns <c>null</c> when the deck contains something that could invalidate the trajectory
/// (a Power card, or a card that grants the player a power — a damage-reducing power we don't model).
/// </summary>
public sealed class DeckProfile
{
    /// <summary>Upper-bound block from one full pass through the deck (front-loaded into the bound).</summary>
    public int BlockPerCycle { get; private init; }

    /// <summary>Player turns it takes to cycle the whole deck once (deck size ÷ cards drawn per turn).</summary>
    public int CycleTurns { get; private init; }

    public int DeckCount { get; private init; }

    /// <summary>A cloneable Weak power captured from a Weak-applying card, or null if the deck can't apply
    /// Weak. Lets the (Search-side) trajectory keep enemies Weak without referencing Content's power types.</summary>
    public PowerModel? WeakProto { get; private init; }

    /// <summary>True when every card's per-play damage was identical across a battery of probe contexts and
    /// the deck grows no damage (no Strength/Vulnerable source) — so <see cref="DamagePerCycle"/> is a valid
    /// UPPER bound on the player's damage output. Gate for the player-damage-budget bounds (loss pruning and
    /// the multi-enemy horizon). False ⇒ those features bail (the survival horizon still works).</summary>
    public bool DamageBoundable { get; private init; }

    /// <summary>Upper-bound single-target damage from one full pass through the deck. Only meaningful when
    /// <see cref="DamageBoundable"/>.</summary>
    public int DamagePerCycle { get; private init; }

    /// <summary>True if any card heals the player mid-combat. Forward HP loss to a forced death equals the
    /// player's current HP only when no healing happens, so the loss-proof gates on this being false.</summary>
    public bool HealsPlayer { get; private init; }

    // Defence-in-depth cushion on the (already-exact-for-boundable-decks) probed damage budget, so a probe
    // battery that misses a state dimension still over-states the player's reach — the safe direction for the
    // damage-budget bounds (it can only SUPPRESS a prune / loosen a horizon, never cause an unsound one).
    private const double DamageCushion = 1.5;
    private const int UnaccountedDamagePerTurn = 10;

    /// <summary>An UPPER bound on the single-target damage the player can deal over <paramref name="turns"/>
    /// player turns (a full deck's damage per cycle, front-loaded, plus cushion). Only meaningful when
    /// <see cref="DamageBoundable"/>. Used by the loss-proof and the multi-enemy horizon to bound kill speed.</summary>
    public long DamageUpperBound(int turns)
    {
        long cycles = (turns + CycleTurns - 1) / CycleTurns;
        return (long)Math.Ceiling(cycles * (double)DamagePerCycle * DamageCushion)
             + (long)UnaccountedDamagePerTurn * turns;
    }

    /// <summary>Build the profile, or null if the deck disqualifies the trajectory entirely.</summary>
    public static DeckProfile? Build(Player player)
    {
        var deck = DeckCards(player).ToList();
        var deckClones = deck.Select(c => c.Clone()).ToList();   // for pile-filling probe contexts

        int blockPerCycle = 0, damagePerCycle = 0;
        bool damageBoundable = true, healsPlayer = false;
        PowerModel? weakProto = null;

        foreach (var card in deck)
        {
            if (card.Type == CardType.Power) return null;        // powers may reduce incoming / scale damage

            if (!TryProbe(card, ProbeContexts.Base, deckClones, out var baseR)) return null;
            if (baseR.GrantsPlayerPower) return null;            // could be a damage-reducing player power
            if (baseR.AppliesVulnerable) damageBoundable = false;// grows our damage ⇒ no valid damage UB
            if (baseR.HealsPlayer) healsPlayer = true;           // breaks "forward loss to death == current HP"

            blockPerCycle += baseR.Block;
            if (baseR.WeakPower != null) weakProto ??= baseR.WeakPower;

            // Damage UB: only valid if the card deals the SAME damage across every probe context (i.e. its
            // damage doesn't scale with combat state). Any divergence ⇒ the deck isn't damage-boundable.
            int dmg = baseR.Damage;
            foreach (var ctx in ProbeContexts.Scaling)
            {
                if (!TryProbe(card, ctx, deckClones, out var r)) { damageBoundable = false; break; }
                if (r.Damage != dmg) { damageBoundable = false; break; }
            }
            damagePerCycle += dmg;
        }

        int deckCount = deck.Count;
        int cycleTurns = Math.Max(1, (deckCount + Player.CardsDrawnPerTurn - 1) / Player.CardsDrawnPerTurn);
        return new DeckProfile
        {
            BlockPerCycle = blockPerCycle,
            DamagePerCycle = damagePerCycle,
            DamageBoundable = damageBoundable,
            HealsPlayer = healsPlayer,
            WeakProto = weakProto,
            CycleTurns = cycleTurns,
            DeckCount = deckCount,
        };
    }

    private static IEnumerable<CardModel> DeckCards(Player p) =>
        p.DrawPile.Concat(p.Hand).Concat(p.DiscardPile);

    // ---------- probing ----------

    /// <summary>What a single controlled play of a card produced.</summary>
    private readonly record struct ProbeResult(
        int Block, int Damage, bool GrantsPlayerPower, PowerModel? WeakPower, bool AppliesVulnerable, bool HealsPlayer);

    /// <summary>A pre-play perturbation of the probe state, used to detect state-dependent damage.</summary>
    private delegate void ProbeContext(CombatState combat, List<CardModel> deckClones);

    private static class ProbeContexts
    {
        public static readonly ProbeContext Base = (_, _) => { };

        // A battery that perturbs every combat-state dimension a state-independent card should ignore: prior
        // block (Body Slam), the unblocked-hit / lost-HP / exhaust counters (Tear Asunder, Spite, Evil Eye),
        // a high turn number, and full piles + hand (Perfected-Strike-style deck-composition scaling). A card
        // whose probed damage moves under ANY of these depends on combat state and can't be upper-bounded
        // from a single probe, so the deck is treated as not damage-boundable (the budget bounds then bail).
        public static readonly ProbeContext[] Scaling =
        {
            (c, _) => c.Player.Block = 200,
            (c, _) => { c.PlayerUnblockedHitsCount = 50; c.PlayerLostHpThisTurn = true; c.CardExhaustedThisTurn = true; },
            (c, _) => c.TurnNumber = 25,
            (c, deck) =>
            {
                foreach (var d in deck) { c.Player.DrawPile.Add(d.Clone()); c.Player.DiscardPile.Add(d.Clone()); }
                foreach (var d in deck) c.Player.ExhaustPile.Add(d.Clone());
            },
            (c, deck) => { foreach (var d in deck) c.Player.Hand.Add(d.Clone()); },
        };
    }

    private static bool TryProbe(CardModel card, ProbeContext ctx, List<CardModel> deckClones, out ProbeResult result)
    {
        result = default;
        if (card.Unplayable) { result = new ProbeResult(0, 0, false, null, false, false); return true; }  // status/curse: inert
        try
        {
            const int dummyHp = 1_000_000;
            const int playerMaxHp = 1_000_000;
            const int playerHp = playerMaxHp - 100_000;   // below max so a mid-combat heal is observable
            var noop = new MoveState("noop", (_, _) => { }, intentDamage: null);
            noop.FollowUp = noop;
            var dummy = new Monster
            {
                Name = "probe", MaxHp = dummyHp, CurrentHp = dummyHp,
                Ai = new MonsterMoveStateMachine(new MonsterState[] { noop }, "noop") { CurrentMoveId = "noop" },
            };
            var player = new Player { Name = "probe", CurrentHp = playerHp, MaxHp = playerMaxHp, MaxEnergy = 99 };
            player.ResetEnergy();
            var copy = card.Clone();
            player.Hand.Add(copy);
            var combat = new CombatState { Player = player, Monsters = { dummy }, TurnNumber = 5 };

            ctx(combat, deckClones);

            int before = dummy.CurrentHp;
            int playerHpBefore = player.CurrentHp;
            CombatManager.PlayCard(combat, copy, card.NeedsTarget ? dummy : null);

            int block = Math.Max(0, player.Block);
            int damage = Math.Max(0, before - dummy.CurrentHp);
            result = new ProbeResult(
                Block: block,
                Damage: damage,
                GrantsPlayerPower: player.Powers.Count > 0,
                WeakPower: dummy.GetPower("Weak")?.Clone(),
                AppliesVulnerable: dummy.GetPowerAmount("Vulnerable") > 0,
                HealsPlayer: player.CurrentHp > playerHpBefore);
            return true;
        }
        catch { return false; }
    }
}
