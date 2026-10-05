"""Encounter pools per act, from the game's act classes (MegaCrit.Sts2.Core.Models.Acts.*.GenerateAllEncounters) and the room-generation rules
(ActModel.GenerateRooms): the first `weak` monster fights of an act come from the weak pool, the rest of its rooms from the regular pool, elites from
the elite pool, one boss from the boss pool. See `.claude/skills/sts2-acts`. Public knowledge a player has: which fight comes next is not.
"""
import re

import sts2

ACTS = {
    "Overgrowth": dict(
        act=0, rooms=15, weak_fights=3,
        weak="FuzzyWurmCrawlerWeak NibbitsWeak ShrinkerBeetleWeak SlimesWeak",
        regular="CubexConstructNormal FlyconidNormal FogmogNormal InkletsNormal MawlerNormal NibbitsNormal OvergrowthCrawlers RubyRaidersNormal SlimesNormal SlitheringStranglerNormal SnappingJaxfruitNormal VineShamblerNormal",
        elite="BygoneEffigyElite ByrdonisElite PhrogParasiteElite", boss="VantomBoss CeremonialBeastBoss TheKinBoss"),
    "Underdocks": dict(
        act=0, rooms=15, weak_fights=3,
        weak="CorpseSlugsWeak SeapunkWeak SludgeSpinnerWeak ToadpolesWeak",
        regular="CorpseSlugsNormal CultistsNormal FossilStalkerNormal GremlinMercNormal HauntedShipNormal LivingFogNormal PunchConstructNormal SeapunkNormal SewerClamNormal TwoTailedRatsNormal",
        elite="PhantasmalGardenersElite SkulkingColonyElite TerrorEelElite", boss="WaterfallGiantBoss SoulFyshBoss LagavulinMatriarchBoss"),
    "Hive": dict(
        act=1, rooms=14, weak_fights=2,
        weak="BowlbugsWeak ExoskeletonsWeak ThievingHopperWeak TunnelerWeak",
        regular="BowlbugsNormal ChompersNormal ExoskeletonsNormal HunterKillerNormal LouseProgenitorNormal MytesNormal OvicopterNormal SlumberingBeetleNormal SpinyToadNormal TheObscuraNormal",
        elite="DecimillipedeElite EntomancerElite InfestedPrismsElite", boss="TheInsatiableBoss KnowledgeDemonBoss KaiserCrabBoss"),
    "Glory": dict(
        act=2, rooms=13, weak_fights=2,
        weak="DevotedSculptorWeak ScrollsOfBitingWeak TurretOperatorWeak",
        regular="AxebotsNormal ConstructMenagerieNormal FabricatorNormal FrogKnightNormal GlobeHeadNormal OwlMagistrateNormal ScrollsOfBitingNormal SlimedBerserkerNormal TheLostAndForgottenNormal",
        elite="KnightsElite MechaKnightElite SoulNexusElite", boss="QueenBoss TestSubjectBoss AeonglassBoss"),
}


def _snake(name):
    return re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", name).upper()


_KNOWN = set(sts2.names()["encounter"])
UNKNOWN = []


def _ids(s):
    out = []
    for n in s.split():
        i = _snake(n)
        if i in _KNOWN:
            out.append(i)
        else:
            UNKNOWN.append((n, i))
    return out


POOLS = {act: {k: (_ids(v) if k in ("weak", "regular", "elite", "boss") else v) for k, v in d.items()} for act, d in ACTS.items()}


def pool(act_name, kind):
    """Encounter ids of `kind` (weak / regular / elite / boss) in the act (Overgrowth, Underdocks, Hive, Glory)."""
    return list(POOLS[act_name][kind])


def act_names(act_index):
    """Act names for the run's act index (0 = Overgrowth or Underdocks, 1 = Hive, 2 = Glory)."""
    return [n for n, d in ACTS.items() if d["act"] == act_index]
