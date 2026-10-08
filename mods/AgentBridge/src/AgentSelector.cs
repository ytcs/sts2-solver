using MegaCrit.Sts2.Core.Entities.CardRewardAlternatives;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.TestSupport;

namespace AgentBridge;

public sealed class AgentSelector : MegaCrit.Sts2.Core.TestSupport.ICardSelector
{
    public static readonly AgentSelector Instance = new();

    public sealed class Pending
    {
        public required List<CardModel> Options;
        public required int Min, Max;
        public string? Prompt;
        public required TaskCompletionSource<IEnumerable<CardModel>> Tcs;
    }

    public static Pending? Current { get; private set; }

    public Task<IEnumerable<CardModel>> GetSelectedCards(IEnumerable<CardModel> options, int minSelect, int maxSelect)
    {
        var list = options.ToList();
        if (list.Count == 0) return Task.FromResult<IEnumerable<CardModel>>(Array.Empty<CardModel>());
        int min = Math.Max(minSelect, PromptPatch.NextMin ?? 0);
        var p = new Pending { Options = list, Min = Math.Min(min, list.Count), Max = Math.Min(Math.Max(maxSelect, min), list.Count), Tcs = new(TaskCreationOptions.RunContinuationsAsynchronously), Prompt = PromptPatch.Next };
        PromptPatch.Next = null;
        PromptPatch.NextMin = null;
        Current = p;
        return p.Tcs.Task;
    }

    public CardRewardSelection GetSelectedCardReward(IReadOnlyList<CardCreationResult> options, IReadOnlyList<CardRewardAlternative> alternatives)
        => new CardRewardSelection { card = options.Count > 0 ? options[0].Card : null, alternative = null };

    public static string? Answer(IReadOnlyList<int> idx)
    {
        var p = Current;
        if (p == null) return "no pending selection";
        if (idx.Count < p.Min || idx.Count > p.Max) return $"pick between {p.Min} and {p.Max} cards";
        if (idx.Any(i => i < 0 || i >= p.Options.Count) || idx.Distinct().Count() != idx.Count) return "bad index";
        Current = null;
        Snap.LogChoose(idx);
        p.Tcs.TrySetResult(idx.Select(i => p.Options[i]).ToList());
        return null;
    }
}
