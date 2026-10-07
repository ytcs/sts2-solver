# Literature review: design questions for the STS2 solver

Scope: seven design questions for an STS2 AI made of a bit-exact Rust combat simulator, a PPO policy/value network with a categorical end-HP outcome head, and a determinized search (sample the hidden draw order / RNG, play the top-M root actions over K futures with the policy, evaluate leaves with the network after 2 turns), plus macro decisions (paths, picks, potions) priced from the outcome head.

Evidence markers: **[verified]** means the claim or number was checked against the paper or its abstract during this review; **[memory]** means it is cited from background knowledge and was not re-checked; **[not found]** means a search turned up nothing usable. Where evidence for *our* setting (single-player, stochastic, hidden draw order, no adversary) is thin, the section says so.

---

## 1. Privileged-information predictors and their transfer to observation-only predictors

**Key work**

- **Suphx** (Li, Koyamada, Ye, Liu, Wang, Yang, Zhao, Qin, Liu, Hon; arXiv 2003.13590, 2020) [verified]. Mahjong. *Oracle guiding*: first train an "oracle" agent that also sees opponents' hands and the wall, then remove the perfect features step by step with a Bernoulli dropout mask whose keep probability γ_t decays from 1 to 0 over training. Once γ = 0 they divide the learning rate by 10 and reject samples whose importance weight exceeds a threshold. Without these tricks "the continual training is not stable and does not lead to further improvements". Ablation order: SL < RL-basic < RL-1 (adds a *global reward predictor*, a GRU that predicts the final game reward from the rounds so far, used to shape per-round rewards as Φ(x_k) − Φ(x_{k−1})) < RL-2 (adds oracle guiding). Online: 8.74 dan stable rank on Tenhou, about 2 dan above earlier AIs. The paper reports the oracle gain as a stable-rank gain in a figure; the exact delta was not extracted.
- **Asymmetric actor-critic** (Pinto, Andrychowicz, Welinder, Zaremba, Abbeel; arXiv 1710.06542, RSS 2018) [verified]. The critic sees the full simulator state and the actor sees images. Training is much faster on simulated manipulation tasks, and the policy transfers sim-to-real with domain randomization.
- **Unbiased asymmetric actor-critic** (Baisero & Amato; arXiv 2105.11674, AAMAS 2022) [verified]. The common variant, a state-only critic V(s) used for a history-based policy, is *biased*: V(s) is not the value of the agent's information state, and the policy gradient theorem does not hold for it. Their fix is a critic on (history, state), V(h, s). It stays unbiased, because E_{s|h}[V(h,s)] = V(h), while still using state information to cut variance. On strongly partially observable domains it converges to better policies and/or faster than both symmetric and biased-asymmetric baselines (the paper's per-domain numbers were not extracted).
- **Learning by Cheating** (Chen, Zhou, Koltun, Krähenbühl; arXiv 1912.12294, CoRL 2019) [verified]. A privileged driving agent that sees the ground-truth map and actors teaches a vision-only student. It was the first method to reach 100% success on every task of the original CARLA benchmark, and cut infractions by an order of magnitude on NoCrash. It works because the teacher's actions are *achievable* from the student's inputs. Driving is close to fully observable from cameras.
- **Imitation gap / ADVISOR** (Weihs et al.; arXiv 2007.12173, NeurIPS 2021) [verified]. When the teacher acts on information the student lacks, imitation marginalizes over that information. The student copies the teacher's average action under its own posterior, which can be much worse than the best observation-only action. A typical case is a teacher that walks straight to a hidden goal, where the best student action is to explore. ADVISOR weights the imitation loss against the RL loss per state, by how well an auxiliary policy can imitate there. It beats pure imitation, pure RL, and their sequential or parallel combinations on gridworld, particle and 3D tasks.
- **Privileged world models and critics**: Hu et al., "Privileged Sensing Scaffolds RL" (Scaffolder, ICLR 2024, arXiv 2405.14853) [verified]. Privileged observations train the world model, critic and reward estimator; the policy stays on target observations. It often matches policies that have the privileged sensors at test time.

**When a privileged teacher helps and when it hurts.** The literature splits cleanly:

1. *Privileged inputs to evaluate the student's own policy* (critics, baselines, value targets): this helps and is sound as long as the critic is conditioned on the student's information too (Baisero & Amato). The privileged input only removes variance. Under the student's posterior, the expected privileged value equals the student's value.
2. *Privileged inputs to the actor whose behaviour is then distilled* (imitation of an oracle policy): this is risky. It helps when the hidden information hardly changes the best action (Learning by Cheating) or when it is annealed away gradually with continued RL (Suphx). It hurts when the best observation-only action is qualitatively different, for example information gathering or hedging (Weihs et al.).
3. A *value of the oracle's own policy* (a clairvoyant who knows the draw order) is **optimistically biased** as a target for the observation-only predictor. This is the same "averaging over clairvoyance" error as PIMC (§2). The bias is largest where knowing the future would change decisions.

**Implication for us.**
(a) A privileged outcome head used as a **leaf evaluator inside the determinized search** has no bias problem *if* it predicts the outcome of the *observation-only policy* from a determinized state. At a leaf the state really is fully known in that sample, and averaging such values over K samples estimates the observation-level value without bias. The categorical head adds a useful property: averaging the K privileged *distributions* gives the correct mixture distribution by the law of total probability.
(b) For training an **observation-only outcome head**, distill from a privileged head of the *same* policy (soft cross-entropy to the teacher's categorical distribution, averaged over posterior samples, or simply as an auxiliary target). Do not distill from a clairvoyant policy's value.
(c) For **PPO**, use a V(o, s) critic with the draw order as extra input, which is unbiased per Baisero & Amato, rather than a V(s)-only critic.
(d) **Distill values, not actions**, from anything that acts on draw order. If an actor must be distilled from a privileged one, use Suphx's γ-decay dropout plus continued RL, with the LR/10 and importance-weight clipping after γ = 0.

---

## 2. Determinization: PIMC, strategy fusion, non-locality, ISMCTS

**Key work**

- **Frank & Basin 1998** ("Search in games with incomplete information: a case study using Bridge card play", AIJ) [memory] name the two failure modes of PIMC. *Strategy fusion*: the search picks a different continuation in each sampled world, although the real player cannot tell the worlds apart. *Non-locality*: a node's value depends on other parts of the tree, through an opponent who steers play toward worlds they know are good for them.
- **Long, Sturtevant, Buro, Furtak**, "Understanding the success of PIMC sampling in game tree search", AAAI 2010 [verified, read in full]. They define three tree properties:
  - *leaf correlation* lc: the probability that sibling terminal nodes have the same payoff;
  - *bias* b: how far the game favours one player;
  - *disambiguation factor* df: how fast information sets shrink as play proceeds.

  On synthetic trees, PIMC is at its worst with **low leaf correlation**, where decisive anti-correlated choices sit near the leaves. Mid-range df is hardest in absolute terms, but PIMC's gain over random play grows steadily with df. With df around 0.7–0.9, PIMC does well even at low correlation, because the game becomes perfect-information quickly. Measured on real games, Skat and Hearts have lc ≈ 0.8–1.0 and df ≈ 0.6, a region where PIMC loses only about 0.1 points per game against a Nash-equilibrium player while gaining about 0.4 over random. In Skat endgames solved with CFR, PIMC's mistakes cost 0.42 tournament points per deal over 3,000 unresolved positions, negligible against a per-tournament standard deviation of 778.
- **Information Set MCTS** (Cowling, Powley, Whitehouse; IEEE TCIAIG 2012) [abstract verified; details from memory]. Builds one tree over *information sets* instead of a tree per determinization, so every sample shares statistics and strategy fusion goes away. Domains (from memory): Lord of the Rings: The Confrontation, Phantom (4,4,4) and Dou Di Zhu. As I recall, ISMCTS beat determinized UCT where strategy fusion matters and roughly matched it in Dou Di Zhu. The exact win rates were not re-verified.
- **Closest modern analogue**: Rubin, "Unsound Search with Policy and Value Networks in Legends of Code and Magic" (arXiv 2609.06816, 2026) [verified abstract]. A determinized search over sampled opponent decks with imitation-trained policy/value nets lifts the battle win rate against the LoCM champion from **26.8% to 51.35%** (+24.6 pp; 95% CI [50.37, 52.33] over 10,000 games). The author argues LoCM is in a region where "defects are cheap" for Monte Carlo methods.

**How this maps to our search.**
- **Non-locality** needs an adversary with private information who steers play. STS2 has none: enemies follow scripted or RNG intents and do not hide information strategically. Only **strategy fusion**, an optimism bias, remains.
- At the **root**, our search picks one action by averaging over the *same* K futures, so it treats the root as an information set, which is correct.
- **Below the root**, play is by a policy that sees only multisets of draw/discard piles. It cannot tailor its play to the sampled order, so that part of the playout has no strategy fusion.
- Fusion can enter in two places:
  1. any search choice *inside* the 2-turn horizon that is made per determinization (for example a per-world max over M actions at depth > 0);
  2. a leaf evaluator that sees the draw order of the next turns while being trained as the value of a clairvoyant policy (§1).
- STS2 combat has a high disambiguation factor: each turn draws about 5 of 10–30 cards and reveals the enemy's intents, so information sets collapse every turn. It probably also has moderate-to-high leaf correlation, since most fights are won or lost with an HP margin rather than on a single coin-flip near the end. By Long et al.'s analysis this is the regime where PIMC is near-optimal.

**Implication for us.** Keep PIMC-at-root plus policy-below; it fits the regime where the literature says determinization works, and ISMCTS's extra machinery is unlikely to pay off. Use **common random numbers**: all M candidate actions should face the *same* K futures, as a paired comparison, which the design already does. Do not add per-world maximisation deeper in the tree. If the search is deepened, branch at the information-set level (ISMCTS-style shared nodes keyed by observation) instead of per sample. The bit-exact simulator makes Long et al.'s properties measurable directly: estimate df and lc from random playouts on the real-run fight set. A cheap fusion check is the gap between the search's predicted value and the realised outcome; optimism concentrated in particular fights shows where fusion bites.

---

## 3. Distributional and categorical outcome prediction; calibration

**Key work**

- **C51** (Bellemare, Dabney, Munos; arXiv 1707.06887, ICML 2017) [memory]: a categorical return distribution on 51 fixed atoms with a projected distributional Bellman target; a large Atari gain over DQN. Follow-ups QR-DQN and IQN use quantiles instead.
- **MuZero** (Schrittwieser et al.; arXiv 1911.08265, Nature 2020) [memory]: value and reward predicted as categorical over 601 bins after an invertible squashing transform h(x), with **two-hot** targets, for scale-robust training.
- **Imani & White 2018** ("Improving regression performance with distributional losses", ICML) [memory]: origin of **HL-Gauss**. The scalar target is replaced by a Gaussian smeared over the bins and trained with cross-entropy. Regression accuracy improves through better gradients, not through modelling the distribution.
- **Farebrother et al.**, "Stop Regressing: Training value functions via classification for scalable deep RL" (arXiv 2403.03950, ICML 2024) [verified]. Cross-entropy value losses beat MSE across Atari (single and multi-task), robotic manipulation, chess *without search*, and a Wordle language agent. **HL-Gauss beats both two-hot and C51**, "despite not modeling the return distribution". There is a consistent ~30% gain when scaling parameters with mixture-of-experts on Atari. Their reading: the benefit comes from the loss (robustness to noisy targets and non-stationarity, better representations), not from distributional semantics.
- **Calibration and scoring** [memory]:
  - Gneiting & Raftery 2007 (JASA), "Strictly proper scoring rules": log loss and CRPS are proper. For *ordered* discrete bins, CRPS becomes the **ranked probability score** (RPS, Epstein 1969), which charges more for mass far from the outcome. Log loss is indifferent to the ordering of the bins.
  - Guo et al. 2017 (ICML), "On calibration of modern neural networks": reliability diagrams, ECE, and temperature scaling.
  - PIT histograms check calibration of the whole predictive distribution.
  - The Brier score, with its reliability/resolution decomposition, suits the binary P(win).

**Implication for us.** The categorical end-HP head is well supported as a training device. The project's own ledger agrees with Farebrother's finding: route-worth *tables* over the head's classes were not better than a linear worth, so most of the value comes from the loss, not from using the distribution downstream. Recommendations:
1. Try **HL-Gauss targets** (σ about 0.75 of a bin width) in place of one-hot or two-hot on the HP bins. It is cheap and is the variant that won in Farebrother et al.
2. If anything downstream uses more than the mean (P(win), tail risk before elites, the HP distribution entering the next fight), score the head *as a distribution*: report **RPS/CRPS** over the HP bins, log loss, the P(win) Brier score with reliability, and a PIT histogram, all on a held-out real-run fight set at the natural (unweighted) scenario distribution. `rl/heads_check.py` already covers MSE, Brier, reliability and coverage; RPS and PIT are the missing pieces.
3. Keep the win/loss mass separate from the HP-bin mass. Losses are a point mass, and smearing HL-Gauss across the win/loss boundary would blur P(win).

---

## 4. Expert iteration and search distillation

**Key work** [memory unless marked]

- **ExIt** (Anthony, Tian, Barber; arXiv 1705.08439, NeurIPS 2017): an apprentice network imitates an MCTS expert, and the expert in turn uses the apprentice; beat MoHex in Hex.
- **AlphaZero** (Silver et al., Science 2018) and **MuZero** (2020): visit-count policy targets and n-step or final-outcome value targets.
- **MuZero Reanalyse** (Schrittwieser et al., "Online and offline RL by planning with a learned model", arXiv 2104.06294, NeurIPS 2021): re-run search with the *current* network on stored trajectories to refresh policy and value targets. Large data-efficiency gains, and it works fully offline. EfficientZero (Ye et al., NeurIPS 2021) relies on it heavily.
- **Gumbel AlphaZero/MuZero** (Danihelka, Guez, Schrittwieser, Silver; ICLR 2022, OpenReview id from memory: bERaNdoegnO; note arXiv 2104.06159 is Muesli, not this paper) [verified abstract]. Sample root actions *without replacement* (Gumbel-top-k), allocate simulations with sequential halving, and pick argmax g(a) + logits(a) + σ(q̂(a)). Policy target: π' = softmax(logits + σ(completed Q)), which is provably a policy improvement even when not every action is visited. Matches the state of the art on Go, chess and Atari, and "significantly improves" with few simulations, still learning at 2 simulations where plain MuZero fails (MiniZero, arXiv 2310.11305, confirms this on 9×9 Go).
- **Sampled MuZero** (Hubert et al.; arXiv 2104.06303, ICML 2021): policy improvement over a sampled subset of a large action space, with a correction for the sampling distribution.
- **Stochastic MuZero** (Antonoglou et al.; ICLR 2022) [verified]: afterstates and chance codes. On **2048** it reaches about 500k average score, matching AlphaZero with a perfect simulator, against about 300k for deterministic MuZero. Matches the state of the art on backgammon.
- **TD-Gammon** (Tesauro 1995, CACM): self-play TD(λ) in a stochastic game; search (shallow expectimax / rollouts) layered on later.

**Mapping.** "Top-M root actions × K futures, then policy playouts and leaf values" is structurally a **Gumbel-style root**: a small sampled candidate set scored by Q estimates, with the policy acting deeper down. With a perfect simulator, Stochastic MuZero's learned chance model is unnecessary; the K sampled futures *are* the chance nodes.

**Implication for us.**
1. **Distillation target** (`rl/distill.py`): use the Gumbel improved policy π' = softmax(logits + σ(completed Q)), with unvisited actions filled in by the network's value. Do not use argmax or a hard search action. This gives a provable improvement with M much smaller than the action count and keeps the policy's spread over actions outside the M.
2. **Value targets from search are biased upward** when they take the max over M noisy Q estimates, the winner's curse or optimizer's curse (Smith & Winkler 2006, Management Science [memory]). Use the realised outcome, the λ-return, or the chosen action re-scored on *independent* futures, not the max of the same K.
3. **Reanalyse**: re-score stored real-run fights with the current network and search to refresh targets, which is cheap given the fast simulator.
4. Gate every distilled model with the existing paired tests. The ExIt/AlphaZero literature assumes search > policy, so check that the search actually improves on the policy for each fight class before distilling (for example the bosses-only gains seen with width 5×32).

---

## 5. Hierarchy: low-level outcome models feeding high-level planning; prior deckbuilder AI

**Frameworks** [memory]

- **Options** (Sutton, Precup, Singh, AIJ 1999).
- **MAXQ** (Dietterich, JAIR 2000): V(parent) = V(subtask) + C(completion). For us this reads: run value = fight outcome composed with the value of the post-fight state. Pricing a macro choice through the outcome head, E_{hp ~ head}[W(hp, deck, floor)], is exactly a MAXQ completion function where W is the "worth" of end HP. The option model only needs the *distribution of the termination state* (end HP, potions used), which is what the categorical head supplies.

**Prior work on STS and similar games**

- **Slay the Spire 1, sts_lightspeed + learned non-combat policy** (HF model card Jialeiv/sts-rl-agent, 2024) [verified]. Combat is played by sts_lightspeed's built-in MCTS (about 50k simulations); a roughly 100k-parameter MLP trained with REINFORCE makes every non-combat decision (path, card rewards, shop, campfire, events). A0 Ironclad, 50 held-out seeds: **14% wins and 42.5 average floors**, against 6% and 31.2 floors for the stock heuristics. This is the only quantitative STS decomposition result found. The sample is small (95% CI on 14% over 50 seeds is about ±10 pp).
- **AgenticSTS** (arXiv 2607.02255, July 2026) [verified]: an LLM agent on STS2 with bounded memory; 3/10 → 6/10 wins with a strategy-memory layer. The authors say this is not statistically decisive. It cites a 16% developer-reported human A0 win rate and zero wins for frontier LLMs on a public benchmark. LLM-based; not comparable in method.
- **bottled_ai** (STS1 Watcher bot) [not found]: no source located; no numbers quoted.
- **Legends of Code and Magic draft** (Vieira, Chaimowicz, Tavares; SBGames 2020, SBC proceedings) [verified abstract]: deep RL draft agents beat the best existing draft agents while building quite different decks. A competition battle agent improved from **10th to 4th** with the learned drafter. Battle and draft were trained separately: draft reward = battle win rate of the fixed battle agent with that deck. This is the same decomposition as ours.
- **MTG drafting**: Ward et al. 2020 (arXiv 2009.00655): on about 100k Draftsim drafts, a deep net predicts human picks better than Naive Bayes or expert heuristics. Bertram et al. 2021 (arXiv 2105.11864): contextual preference ranking, a Siamese net trained on 17lands data, predicts human picks given the current deck. These imitate *humans* rather than optimise win rate. 17lands' GIH WR (games-in-hand win rate) is an observational, confounded card value: it reflects which decks pick the card as well as the card itself.
- **Hearthstone**: the AAIA'18 deck win-rate prediction challenge (Annals CSIS vol. 15) [verified abstract] learned deck embeddings to predict win rate from the list, the closest analogue to pricing deck changes through a fight-outcome model.
- **Dominion**: "Playing various strategies in Dominion with deep RL" (AIIDE 2023) and "Dominion: a new frontier for AI research" (arXiv 2405.06846, 2024) [verified abstract]: multiset/set-based state representations; SAC with variable action sets; reported as the first learning agent to make every decision without heuristics. Representing the deck as a multiset matches our policy's draw-pile view.
- **Monster Train, Balatro** [not found]: no peer-reviewed AI work of substance. Balatro has PPO student projects and LLM bots only.

**Implication for us.** The decomposition "strong combat solver + learned or priced macro layer" is the one that has worked in STS (14% vs 6%) and in LoCM (10th → 4th). Our version prices macro choices through a calibrated *termination-state distribution* rather than learning them end-to-end, which is closer to MAXQ and more sample-efficient. The remaining gap is the **completion function W**, the value of HP, gold, potions and deck at floor f. Two cheap estimators are worth comparing:
- **Monte Carlo**: whole-act rollouts in the simulator under the macro policy.
- **TD-trained**: a W(hp, ...) head fitted to realised run outcomes.

Neither the literature nor our ledger yet shows that a richer W beats linear, so keep linear as the default and test any replacement with the paired gate.

---

## 6. Conditioning a predictor on control inputs (allowed consumables)

**Key work** [memory]

- **UVFA** (Schaul, Horgan, Gregor, Silver; ICML 2015): one approximator V(s, g) over states and goals, which generalises to unseen goals through shared structure.
- **Hindsight Experience Replay** (Andrychowicz et al., NeurIPS 2017): relabels goals to get more conditioned targets.
- **Successor features and GPI** (Barreto et al., NeurIPS 2017): for a new task, act greedily over several policies' values.
- **Option-conditioned values** in MAXQ/options. Behaviour-conditioned value functions in general (for example "policy-conditioned value functions", Harb et al. 2020 [memory]).

**What the literature implies.**
1. A conditioned value V(s, c) is only meaningful if the behaviour under c *actually follows* c. Condition the policy and the value together, train on c sampled at random (including the configurations used at decision time), and have the search use the same c in its playouts.
2. **Amortisation shrinks differences.** Shared function approximation pulls V(s, c1) and V(s, c2) toward each other, so the *difference* (the "price" of allowing a potion) is biased toward zero and noisier than either value. This is the known weakness of amortised counterfactuals; UVFA papers report generalisation of values, not accuracy of differences.
3. For decisions that hinge on the difference, **estimate it directly by paired simulation**: the same seeds and futures with and without the potion, which is common random numbers again. Use the conditioned network as a prior or control variate, not as the final answer.

**Implication for us.** Keep potion/consumable masks as a *joint* input to policy and outcome head, sampled at random during training. Price potions by paired search or simulation on identical futures (the M2 payoff test already does this). Our finding that "priced = free because the policy throws ~98% of potions" is a textbook case of point 1: the conditioning input has no effect because behaviour ignores it. Fix the behaviour first (a potion-use head trained with the mask as input, and a penalty or price for use) before any conditioned value can carry a price signal.

---

## 7. Curriculum and prioritised scenario sampling

**Key work**

- **PLR** (Jiang, Grefenstette, Rocktäschel; arXiv 2010.03934, ICML 2021) [verified]: replay levels in proportion to a learning-potential score (mean positive TD error / GAE magnitude) with a staleness term. Better sample efficiency and generalisation on Procgen; combined with UCB-DrAC it gives over 76% improvement in test return over standard baselines.
- **Robust PLR** (Jiang et al., "Replay-guided adversarial environment design", arXiv 2110.02439, NeurIPS 2021) [memory]: training *only* on replayed (curated) levels, with no gradient from fresh random levels, improves robustness and gives a minimax-regret guarantee at equilibrium.
- **PAIRED** (Dennis et al.; arXiv 2012.02096, NeurIPS 2020) [memory]: a teacher generates levels that maximise regret between an antagonist and the protagonist. Hard to train in practice.
- **ACCEL** (Parker-Holder et al.; arXiv 2203.01302, ICML 2022) [verified]: PLR plus small *edits* to high-regret levels; state of the art on BipedalWalker, MiniGrid and CarRacing.
- **"No Regrets"** (Rutherford, Beukman, Willi, Lacerda, Hawes, Foerster; arXiv 2408.15099, NeurIPS 2024) [verified]. **Key result for us**: the regret proxies used by PLR/ACCEL (positive value loss, MaxMC) do *not* track regret. They mostly track success rate, so much of the sampled experience does not help learning. They propose **learnability = p(1−p)** (p = success rate on the level) and **Sampling For Learnability (SFL)**, which mixes a buffer of high-learnability levels with uniform samples. It gives more robust policies on a new worst-α% evaluation protocol.
- **Data selection**: RHO-LOSS (Mindermann et al., ICML 2022) [memory]: prioritise points that are learnable, worth learning, and not yet learnt; skip noisy and already-mastered points.

**Pitfalls**
1. Score functions correlate with difficulty rather than learnability (Rutherford et al.).
2. Forgetting easy or common cases when they are under-sampled; SFL keeps a uniform fraction for this reason.
3. **Value and outcome calibration shifts with the sampling distribution.** A head trained on a reweighted mix learns the reweighted base rates, so P(win) is biased on the natural distribution unless the head loss is importance-weighted back, or calibration is checked and fixed on unweighted data.
4. Stale scores: priorities must be refreshed as the policy improves; PLR uses a staleness term.

**Implication for us.** `ppo --adaptive` (fights won 20–80% of the time weigh 1) is already a learnability scheme, consistent with the strongest recent evidence. Recommendations:
- Use p(1−p) or a smooth version of it rather than a hard band.
- Keep a uniform or natural-distribution fraction (SFL-style, for example 20–50%) so common easy fights are not forgotten.
- Refresh p regularly.
- **Importance-weight the outcome-head loss** (or recalibrate it on unweighted data), because the macro layer reads P(win) and E[end HP] as absolute numbers.
- Evaluate on the natural real-run fight set and on a worst-α% slice, as Rutherford et al. do, not only on the weighted training mix.

---

## Summary of recommendations

1. **Privileged information**: use it in critics and leaf evaluators that estimate the value of the *observation-only* policy (V(o, s), as Baisero & Amato show is unbiased). Never use a clairvoyant policy's value as a target. Distill values, not actions; if an actor must be distilled, use Suphx-style γ-annealing plus continued RL.
2. **Determinization**: keep PIMC-at-root plus a multiset-only policy below. STS2 has no adversary (so no non-locality) and a high disambiguation factor, the regime where PIMC is near-optimal (Long et al. 2010; LoCM +24.6 pp). Use common random numbers across candidate actions, avoid per-world maximisation inside the horizon, and measure df and lc on our own simulator.
3. **Categorical head**: try HL-Gauss targets. Score the distribution with RPS/CRPS, log loss, a reliability diagram for P(win) and PIT, on the natural distribution. Expect gains from the loss more than from using the distribution downstream (Farebrother 2024, consistent with our worth-table result).
4. **Search distillation**: use the Gumbel improved-policy target softmax(logits + σ(completed Q)) over the M searched actions. Avoid max-of-noisy-Q value targets (winner's curse). Use Reanalyse on stored fights.
5. **Hierarchy**: the solver + priced macro layer matches the decompositions that worked in STS1 (14% vs 6% A0) and LoCM (10th → 4th). The open problem is the completion function W; keep it linear until a paired test says otherwise.
6. **Control-conditioned values**: condition policy and value jointly on random masks, and price consumables by paired simulation; amortised differences shrink toward zero. Fix potion *behaviour* before expecting a price signal.
7. **Curriculum**: p(1−p) learnability sampling with a uniform fraction (SFL). Importance-weight or recalibrate the outcome head for the natural distribution, and evaluate on worst-α% fights.

## Evidence gaps

- No published work studies privileged-teacher distillation in a single-player game with hidden draw order specifically.
- The ISMCTS numbers and the per-domain numbers of Baisero & Amato were not re-verified.
- No peer-reviewed STS2 combat-solver results were found.
- The only STS1 decomposition number comes from a 50-seed model card.
