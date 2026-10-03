//! Prints the size of the combat state and its biggest components (memory budget work).
use std::mem::size_of;
use sts2sim::engine::*;
use sts2sim::state::*;

fn main() {
    println!("Combat            {}", size_of::<Combat>());
    println!("Creature          {} x {}", size_of::<Creature>(), MAX_CREATURES);
    println!("MonsterState      {}", size_of::<MonsterState>());
    println!("Power             {} x {} per creature", size_of::<Power>(), MAX_POWERS);
    println!("Card              {} x {}", size_of::<Card>(), MAX_CARDS);
    println!("PlayerState       {}", size_of::<PlayerState>());
    println!("Pile              {}", size_of::<Pile>());
    println!("Relic             {} x {}", size_of::<Relic>(), MAX_RELICS);
    println!("HistLog           {}", size_of::<HistLog>());
    println!("HistEntry         {}", size_of::<HistEntry>());
    println!("History           {}", size_of::<History>());
    println!("Decision          {}", size_of::<Decision>());
    println!("Option<Decision>  {}", size_of::<Option<Decision>>());
    println!("Snapshot          {}", size_of::<Snapshot>());
    println!("AutoQueue         {}", size_of::<AutoQueue>());
    println!("PlayCtx           {}", size_of::<PlayCtx>());
    println!("RngSet            {}", size_of::<RngSet>());
    println!("ActionBuf         {}", size_of::<ActionBuf>());
}
