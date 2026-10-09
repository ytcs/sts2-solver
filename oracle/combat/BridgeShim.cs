// What the shared mod sources (mods/AgentBridge/src/{Text,Snap,AgentSelector,PromptPatch}.cs) take from the mod's GlobalUsings.cs and Main.cs.
global using MegaCrit.Sts2.Core.Combat;
global using MegaCrit.Sts2.Core.Context;
global using MegaCrit.Sts2.Core.Entities.Cards;
global using MegaCrit.Sts2.Core.Entities.Creatures;
global using MegaCrit.Sts2.Core.Entities.Players;
global using MegaCrit.Sts2.Core.Models;
global using MegaCrit.Sts2.Core.MonsterMoves.Intents;
global using MegaCrit.Sts2.Core.Runs;

namespace AgentBridge
{
    public static class Main
    {
        public static readonly MegaCrit.Sts2.Core.Logging.Logger Log = new("AgentBridge", MegaCrit.Sts2.Core.Logging.LogType.Generic);
    }
}
