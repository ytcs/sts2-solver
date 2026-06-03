using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Colorless card registration. A partial of Catalog; Core's CardTables()
// aggregates ColorlessCardFactories alongside the other characters'. Colorless
// cards belong to no single character (they appear via card-reward/relic
// effects), so there is no starter deck here.
// ===========================================================================
public static partial class Catalog
{
    internal static readonly Dictionary<string, Func<CardModel>> ColorlessCardFactories = new(StringComparer.OrdinalIgnoreCase)
    {
        // Attacks (damage-faithful).
        ["FlashOfSteel"] = () => new FlashOfSteel(),
        ["DramaticEntrance"] = () => new DramaticEntrance(),
        ["MindBlast"] = () => new MindBlast(),
        ["HandOfGreed"] = () => new HandOfGreed(),
        ["Clash"] = () => new Clash(),
        // Skills (block / draw / debuff — HP-faithful).
        ["Finesse"] = () => new Finesse(),
        ["DarkShackles"] = () => new DarkShackles(),
        ["MasterOfStrategy"] = () => new MasterOfStrategy(),
        ["ThinkingAhead"] = () => new ThinkingAhead(),
        ["Impatience"] = () => new Impatience(),
        ["PanicButton"] = () => new PanicButton(),
        // Powers.
        ["Panache"] = () => new Panache(),
        ["TheBomb"] = () => new TheBomb(),
        ["Mayhem"] = () => new Mayhem(),
        // HP-neutral / RNG cards ported as their faithful subset (documented inert).
        ["Apotheosis"] = () => new Apotheosis(),
        ["Metamorphosis"] = () => new Metamorphosis(),
        ["Enlightenment"] = () => new Enlightenment(),
        ["Purity"] = () => new Purity(),

        // ---- Second wave: the 39 remaining Colorless pool cards + the Ancient Wish. ----
        // Attacks.
        ["Bolas"] = () => new Bolas(),
        ["Fisticuffs"] = () => new Fisticuffs(),
        ["GoldAxe"] = () => new GoldAxe(),
        ["Jackpot"] = () => new Jackpot(),
        ["Omnislice"] = () => new Omnislice(),
        ["Rend"] = () => new Rend(),
        ["Salvo"] = () => new Salvo(),
        ["SeekerStrike"] = () => new SeekerStrike(),
        ["ThrummingHatchet"] = () => new ThrummingHatchet(),
        ["UltimateStrike"] = () => new UltimateStrike(),
        ["Volley"] = () => new Volley(),
        // Skills.
        ["Alchemize"] = () => new Alchemize(),
        ["Anointed"] = () => new Anointed(),
        ["BeatDown"] = () => new BeatDown(),
        ["Catastrophe"] = () => new Catastrophe(),
        ["Discovery"] = () => new Discovery(),
        ["Equilibrium"] = () => new Equilibrium(),
        ["HiddenGem"] = () => new HiddenGem(),
        ["JackOfAllTrades"] = () => new JackOfAllTrades(),
        ["Production"] = () => new Production(),
        ["Prolong"] = () => new Prolong(),
        ["Restlessness"] = () => new Restlessness(),
        ["Scrawl"] = () => new Scrawl(),
        ["SecretTechnique"] = () => new SecretTechnique(),
        ["SecretWeapon"] = () => new SecretWeapon(),
        ["Shockwave"] = () => new Shockwave(),
        ["Splash"] = () => new Splash(),
        ["TheGambit"] = () => new TheGambit(),
        ["UltimateDefend"] = () => new UltimateDefend(),
        ["Wish"] = () => new Wish(),
        // Powers.
        ["Automation"] = () => new Automation(),
        ["Calamity"] = () => new Calamity(),
        ["Entropy"] = () => new Entropy(),
        ["EternalArmor"] = () => new EternalArmor(),
        ["Fasten"] = () => new Fasten(),
        ["Nostalgia"] = () => new Nostalgia(),
        ["PrepTime"] = () => new PrepTime(),
        ["Prowess"] = () => new Prowess(),
        ["RollingBoulder"] = () => new RollingBoulder(),
        ["Stratagem"] = () => new Stratagem(),

        // ---- Multiplayer-only Colorless cards (single-player projection; see ColorlessCards.cs). ----
        ["BeaconOfHope"] = () => new BeaconOfHope(),
        ["BelieveInYou"] = () => new BelieveInYou(),
        ["Coordinate"] = () => new Coordinate(),
        ["GangUp"] = () => new GangUp(),
        ["HuddleUp"] = () => new HuddleUp(),
        ["Intercept"] = () => new Intercept(),
        ["Knockdown"] = () => new Knockdown(),
        ["Lift"] = () => new Lift(),
        ["Mimic"] = () => new Mimic(),
        ["Rally"] = () => new Rally(),
        ["TagTeam"] = () => new TagTeam(),
    };
}
