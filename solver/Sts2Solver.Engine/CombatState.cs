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

    /// <summary>Attack cards the player has played this turn (Necrobinder Lethality boosts the first one).
    /// Reset at the player's turn start, incremented after each Attack resolves.</summary>
    public int AttacksPlayedThisTurn;

    /// <summary>Ethereal cards the player has played this combat (Necrobinder Pull from Below hits 1 per
    /// such play; Banshee's Cry's cost drops). Never reset.</summary>
    public int EtherealPlayedThisCombat;

    /// <summary>Osty attacks resolved this turn (Necrobinder Flatten costs 0 after one; Rattle hits 1 +
    /// this many). Reset at the player's turn start.</summary>
    public int OstyAttacksThisTurn;

    /// <summary>True once the player has applied Doom this turn (Necrobinder Death's Door triples its
    /// block). Reset at the player's turn start.</summary>
    public bool DoomAppliedThisTurn;

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
            EtherealPlayedThisCombat = EtherealPlayedThisCombat,
            OstyAttacksThisTurn = OstyAttacksThisTurn,
            DoomAppliedThisTurn = DoomAppliedThisTurn,
        };
    }

    // NOTE: PlayerHpLost is deliberately excluded — the solver computes *forward* (additional) HP loss,
    // which is independent of how much has already been lost, so states that differ only in accumulated
    // loss share a value and memoise together.
    public string StateKey()
    {
        var monsters = string.Join(";", Monsters.Select(m => m.StateKey()));
        // Necrobinder-only counters: appended only when non-zero so every other character's key is unchanged.
        var necro = (AttacksPlayedThisTurn != 0 ? $"/a{AttacksPlayedThisTurn}" : "")
                  + (EtherealPlayedThisCombat != 0 ? $"/et{EtherealPlayedThisCombat}" : "")
                  + (OstyAttacksThisTurn != 0 ? $"/oa{OstyAttacksThisTurn}" : "")
                  + (DoomAppliedThisTurn ? "/da" : "");
        return $"T{TurnNumber}/{CurrentSide}{(CardExhaustedThisTurn ? "x" : "")}{(PlayerLostHpThisTurn ? "h" : "")}/u{PlayerUnblockedHitsCount}{necro}|{Player.StateKey()}|{monsters}";
    }
}
