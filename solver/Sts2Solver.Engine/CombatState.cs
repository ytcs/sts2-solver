namespace Sts2Solver.Engine;

/// <summary>
/// The full combat state: the player, the monsters, and the turn/side counters. Cloneable for tree
/// search and reducible to a canonical key for memoisation.
/// </summary>
public sealed class CombatState
{
    public Player Player = null!;
    public List<Monster> Monsters = new();

    public int TurnNumber;                       // player turn counter, starts at 1
    public CombatSide CurrentSide = CombatSide.Player;

    /// <summary>Ambient RNG for effects that draw cards mid-turn (Shrug It Off, Pommel Strike, …). When
    /// null (the trace-validator's replay mode), mid-turn card draws are no-ops — the validator supplies
    /// the real hand at each turn start and constructs any mid-turn-drawn card when it is played. A
    /// concrete driver / unit test sets it to make draws real. NOT carried into Clone (search models draw
    /// as an explicit chance node) nor included in the state key (transient driver state).</summary>
    public Rng? Rng;

    /// <summary>Total HP the player has lost across this combat (incremental cost for the solver).</summary>
    public int PlayerHpLost;

    /// <summary>True if a card has been exhausted during the current player turn (reset at the player's
    /// turn start). Evil Eye doubles its block and Forgotten Ritual refunds energy when this is set.</summary>
    public bool CardExhaustedThisTurn;

    /// <summary>True if the player took unblocked damage during their OWN turn (self-damage cards), reset
    /// at the player's turn start — matching the game's same-round-same-side "this turn" check. Spite
    /// multi-hits when this is set.</summary>
    public bool PlayerLostHpThisTurn;

    /// <summary>Number of times the player has taken unblocked damage this combat (any source). Tear Asunder
    /// hits 1 + this many times. (Game: count of the player's DamageReceivedEntry with UnblockedDamage > 0.)</summary>
    public int PlayerUnblockedHitsCount;

    /// <summary>Skills the player has played during the current turn (reset at the player's turn start).
    /// Regent's Lunar Blast hits once per skill played this turn. (Game: CardPlaysFinished this-turn count.)</summary>
    public int SkillsPlayedThisTurn;

    /// <summary>Stars the player has gained during the current turn (reset at the player's turn start).
    /// Regent's Radiate hits once per star gained this turn. (Game: StarsModifiedEntry this-turn sum.)</summary>
    public int StarsGainedThisTurn;

    /// <summary>Number of Attacks the player has finished playing during the current turn (reset at the player's
    /// turn start). Finisher hits this many times; Necrobinder Lethality boosts the first. (Game: count of this
    /// turn's Attack CardPlaysFinished.)</summary>
    public int AttacksPlayedThisTurn;

    /// <summary>Number of cards the player has discarded mid-turn during the current turn (reset at the
    /// player's turn start; the end-of-turn hand discard is not counted). Memento Mori scales on this.</summary>
    public int CardsDiscardedThisTurn;

    /// <summary>Total cards the player has drawn this combat (turn-start hand draws + mid-turn draws). Murder
    /// scales on this. Only tracked + hashed when <see cref="TracksCardsDrawn"/> is set (a deck contains a
    /// card that reads it), so the common case isn't fragmented by an ever-growing counter.</summary>
    public int CardsDrawnThisCombat;

    /// <summary>Set at combat setup when the deck contains a card whose value depends on cumulative cards
    /// drawn (Murder). Gates <see cref="CardsDrawnThisCombat"/> tracking + hashing.</summary>
    public bool TracksCardsDrawn;

    /// <summary>Ethereal cards the player has played this combat (Necrobinder Pull from Below hits 1 per such
    /// play; Banshee's Cry's cost drops). Never reset.</summary>
    public int EtherealPlayedThisCombat;

    /// <summary>Osty attacks resolved this turn (Necrobinder Flatten costs 0 after one; Rattle hits 1 +
    /// this many). Reset at the player's turn start.</summary>
    public int OstyAttacksThisTurn;

    /// <summary>True once the player has applied Doom this turn (Necrobinder Death's Door triples its
    /// block). Reset at the player's turn start.</summary>
    public bool DoomAppliedThisTurn;

    /// <summary>Cards a mid-turn effect has requested to draw while in SEARCH mode (<see cref="Rng"/> null):
    /// <see cref="Cmd.Draw"/> accumulates the requested count here instead of drawing, and the solver resolves
    /// it as an explicit draw chance node immediately after the play (the drawn hand — and any choice on it —
    /// then becomes real in search). Always 0 at a memoised decision state (the solver drains it before
    /// recursing and at every turn boundary), so it is deliberately excluded from <see cref="StateKey"/> /
    /// HashKey. With a concrete <see cref="Rng"/> (rollouts, trace replay) it stays 0 — draws resolve eagerly.</summary>
    public int PendingDraw;

    /// <summary>The card whose deferred draw is pending, when it also has a POST-draw step (EscapePlan's
    /// conditional block, Acrobatics/Prepared's discard-of-choice). After the draw chance node resolves the
    /// solver runs the step (<see cref="CombatManager.ApplyPostDraw"/>) on the drawn hand. Transient like
    /// <see cref="PendingDraw"/> — carried by reference (the immutable card instance) but drained before any
    /// memoised decision state, so NOT hashed. Null for plain terminal draws (ShrugItOff).</summary>
    public CardModel? PendingDrawCard;

    /// <summary>The player's hand count captured the moment a post-draw card deferred its draw
    /// (<see cref="Cmd.DeferDrawThenResolve"/>), so <see cref="CombatManager.ApplyPostDraw"/> can tell how many
    /// cards the draw ACTUALLY produced (hand minus this). EscapePlan grants block only if its draw drew a card
    /// (the game checks the DRAWN card, which is null when both piles are empty — without this guard the
    /// no-draw outcome would optimistically read a pre-existing <c>Hand[^1]</c> Skill). Transient like
    /// <see cref="PendingDraw"/> — set at defer, consumed in ApplyPostDraw before any decision state, NOT hashed.</summary>
    public int PendingDrawHandBefore;

    /// <summary>Cards the player must still DISCARD of their choice this turn (Acrobatics 1, Prepared N) — a
    /// genuine post-draw decision: unlike <see cref="PendingDraw"/> this IS a property of a memoised decision
    /// state (the player picks which to drop), so it is hashed. The solver resolves it one card at a time as a
    /// MAX over the distinct hand cards before normal play resumes.</summary>
    public int PendingDiscard;

    /// <summary>The card whose discard-of-choice is in flight when it ALSO has a step that must run AFTER the
    /// discards resolve (HiddenDaggers: discard 2 of choice, THEN add 2 Shivs — the game's order, so the Shivs
    /// are never in the discard pool). Once <see cref="PendingDiscard"/> drains to 0 (or the hand empties), the
    /// solver runs <see cref="CardModel.OnPostDiscard"/> (<see cref="CombatManager.ApplyPostDiscard"/>) and clears
    /// this. Hashed alongside PendingDiscard when set, so an in-flight HiddenDaggers discard can't collide with a
    /// continuation-less Acrobatics/Prepared discard of an otherwise-identical state. Null for the common
    /// no-continuation discard. With a concrete <see cref="Rng"/> the card resolves eagerly (this stays null).</summary>
    public CardModel? PendingDiscardCard;

    /// <summary>Cards played during the current player turn (reset at turn start, bumped in
    /// <see cref="CombatManager.PlayCard"/>). Only TRACKED + hashed when <see cref="BoundsPlays"/> is set; it
    /// then caps plays per turn (<see cref="MaxPlaysPerTurn"/>) so a cost-0 replayable draw cantrip (EscapePlan,
    /// Prepared — free play + draw, recirculated by reshuffle) can't build an unbounded play chain that blows
    /// the search stack. The cap sits far above any real line, so it is value-preserving.</summary>
    public int PlaysThisTurn;

    /// <summary>Set at setup when the deck holds a card that could loop the per-turn play chain (a cost-0
    /// replayable draw card — <see cref="CardModel.LoopRiskDraw"/>). Gates <see cref="PlaysThisTurn"/>
    /// tracking + hashing + the cap, so the common deck's state space is never fragmented by a play counter.</summary>
    public bool BoundsPlays;

    /// <summary>Per-turn play cap applied only to <see cref="BoundsPlays"/> decks. Generous — above any real
    /// turn's play count — so it bounds only pathological cantrip loops, never an optimal line.</summary>
    public const int MaxPlaysPerTurn = 40;

    public IEnumerable<Monster> LivingMonsters => Monsters.Where(m => m.IsAlive);

    /// <summary>Living enemies a player/Osty attack can target (the monster list, minus the dead).</summary>
    public IEnumerable<Creature> HittableEnemies => Monsters.Where(m => m.IsAlive);

    public IEnumerable<Creature> AllCreatures
    {
        get
        {
            yield return Player;
            if (Player.Osty != null) yield return Player.Osty;   // player-side pet; its powers join the pipeline
            foreach (var m in Monsters) yield return m;
        }
    }

    /// <summary>All powers in play, used by the damage/block pipelines. Order is irrelevant
    /// (additive sums and multiplicative products commute).</summary>
    public IEnumerable<PowerModel> AllPowers => AllCreatures.SelectMany(c => c.Powers);

    public bool PlayerDead => !Player.IsAlive;
    public bool AllMonstersDead => Monsters.All(m => !m.IsAlive);
    public bool IsCombatOver => PlayerDead || AllMonstersDead;

    /// <summary>Diagnostic counter: total <see cref="Clone"/> calls since process start (or last reset). Used
    /// by the `--profile` CLI to attribute solve time to per-node cloning. A single non-atomic increment on the
    /// search hot path — negligible beside the allocation it accompanies, and the search is single-threaded.</summary>
    public static long ClonesCreated;

    public CombatState Clone()
    {
        ClonesCreated++;
        var monsters = new List<Monster>(Monsters.Count);
        foreach (var m in Monsters) monsters.Add((Monster)m.Clone());
        return new CombatState
        {
            Player = (Player)Player.Clone(),
            Monsters = monsters,
            TurnNumber = TurnNumber,
            CurrentSide = CurrentSide,
            PlayerHpLost = PlayerHpLost,
            CardExhaustedThisTurn = CardExhaustedThisTurn,
            PlayerLostHpThisTurn = PlayerLostHpThisTurn,
            PlayerUnblockedHitsCount = PlayerUnblockedHitsCount,
            SkillsPlayedThisTurn = SkillsPlayedThisTurn,
            StarsGainedThisTurn = StarsGainedThisTurn,
            AttacksPlayedThisTurn = AttacksPlayedThisTurn,
            CardsDiscardedThisTurn = CardsDiscardedThisTurn,
            CardsDrawnThisCombat = CardsDrawnThisCombat,
            TracksCardsDrawn = TracksCardsDrawn,
            EtherealPlayedThisCombat = EtherealPlayedThisCombat,
            OstyAttacksThisTurn = OstyAttacksThisTurn,
            DoomAppliedThisTurn = DoomAppliedThisTurn,
            PendingDraw = PendingDraw,
            PendingDrawCard = PendingDrawCard,   // immutable card instance — shared by reference is safe
            PendingDrawHandBefore = PendingDrawHandBefore,
            PendingDiscard = PendingDiscard,
            PendingDiscardCard = PendingDiscardCard,   // immutable card instance — shared by reference is safe
            PlaysThisTurn = PlaysThisTurn,
            BoundsPlays = BoundsPlays,
        };
    }

    // NOTE: PlayerHpLost is deliberately excluded — the solver computes *forward* (additional) HP loss,
    // which is independent of how much has already been lost, so states that differ only in accumulated
    // loss share a value and memoise together.
    public string StateKey()
    {
        var monsters = string.Join(";", Monsters.Select(m => m.StateKey()));
        // Per-turn / per-combat counters contribute only when non-zero, so decks that don't use them keep
        // canonical keys. AttacksPlayedThisTurn is shared (Finisher + Necrobinder) so it appears once, in the
        // `silent` group; the `necro` group adds only the Necrobinder-specific Ethereal/Osty/Doom counters.
        var regent = (SkillsPlayedThisTurn != 0 || StarsGainedThisTurn != 0) ? $"/sk{SkillsPlayedThisTurn}sg{StarsGainedThisTurn}" : "";
        var silent = (AttacksPlayedThisTurn != 0 || CardsDiscardedThisTurn != 0) ? $"/a{AttacksPlayedThisTurn}d{CardsDiscardedThisTurn}" : "";
        var necro = (EtherealPlayedThisCombat != 0 ? $"/et{EtherealPlayedThisCombat}" : "")
                  + (OstyAttacksThisTurn != 0 ? $"/oa{OstyAttacksThisTurn}" : "")
                  + (DoomAppliedThisTurn ? "/da" : "");
        var drawn = TracksCardsDrawn ? $"/w{CardsDrawnThisCombat}" : "";
        var disc = PendingDiscard != 0 ? $"/pd{PendingDiscard}{(PendingDiscardCard != null ? "+" + PendingDiscardCard.StateKey() : "")}" : "";   // discard-of-choice in flight (+continuation card, e.g. HiddenDaggers)
        var plays = BoundsPlays ? $"/np{PlaysThisTurn}" : "";           // per-turn play count (loop-risk decks only)
        disc += plays;
        return $"T{TurnNumber}/{CurrentSide}{(CardExhaustedThisTurn ? "x" : "")}{(PlayerLostHpThisTurn ? "h" : "")}/u{PlayerUnblockedHitsCount}{regent}{silent}{necro}{drawn}{disc}|{Player.StateKey()}|{monsters}";
    }
}
