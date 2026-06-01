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

    /// <summary>Number of Attacks the player has finished playing during the current turn (reset at the
    /// player's turn start). Finisher hits this many times. (Game: count of this turn's Attack CardPlaysFinished.)</summary>
    public int AttacksPlayedThisTurn;

    /// <summary>Number of cards the player has discarded mid-turn during the current turn (reset at the
    /// player's turn start; the end-of-turn hand discard is not counted). Memento Mori scales on this.</summary>
    public int CardsDiscardedThisTurn;

    public IEnumerable<Monster> LivingMonsters => Monsters.Where(m => m.IsAlive);

    public IEnumerable<Creature> AllCreatures
    {
        get
        {
            yield return Player;
            foreach (var m in Monsters) yield return m;
        }
    }

    /// <summary>All powers in play, used by the damage/block pipelines. Order is irrelevant
    /// (additive sums and multiplicative products commute).</summary>
    public IEnumerable<PowerModel> AllPowers => AllCreatures.SelectMany(c => c.Powers);

    public bool PlayerDead => !Player.IsAlive;
    public bool AllMonstersDead => Monsters.All(m => !m.IsAlive);
    public bool IsCombatOver => PlayerDead || AllMonstersDead;

    public CombatState Clone()
    {
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
            AttacksPlayedThisTurn = AttacksPlayedThisTurn,
            CardsDiscardedThisTurn = CardsDiscardedThisTurn,
        };
    }

    // NOTE: PlayerHpLost is deliberately excluded — the solver computes *forward* (additional) HP loss,
    // which is independent of how much has already been lost, so states that differ only in accumulated
    // loss share a value and memoise together.
    public string StateKey()
    {
        var monsters = string.Join(";", Monsters.Select(m => m.StateKey()));
        return $"T{TurnNumber}/{CurrentSide}{(CardExhaustedThisTurn ? "x" : "")}{(PlayerLostHpThisTurn ? "h" : "")}/u{PlayerUnblockedHitsCount}/a{AttacksPlayedThisTurn}/d{CardsDiscardedThisTurn}|{Player.StateKey()}|{monsters}";
    }
}
