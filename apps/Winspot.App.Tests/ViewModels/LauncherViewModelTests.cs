using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Threading;
using System.Threading.Tasks;

using Microsoft.VisualStudio.TestTools.UnitTesting;

using Winspot_App.Models;
using Winspot_App.Services;
using Winspot_App.ViewModels;

namespace Winspot_App.Tests.ViewModels;

[TestClass]
public sealed class LauncherViewModelTests
{
    [TestMethod]
    public void IsExpanded_WhenCreated_ReturnsFalse()
    {
        var viewModel = new LauncherViewModel(new FakeWinspotIpcClient());

        Assert.IsFalse(viewModel.IsExpanded);
    }

    [TestMethod]
    public void HotkeyHint_WhenNoBindingProvided_UsesDefaultChord()
    {
        var viewModel = new LauncherViewModel(new FakeWinspotIpcClient());

        Assert.AreEqual("Ctrl Alt Space", viewModel.HotkeyHint);
    }

    [TestMethod]
    public void HotkeyHint_ReflectsConfiguredBinding()
    {
        var hotkey = new HotkeyBinding { Key = "K", Modifiers = new List<string> { "Control", "Shift" } };

        var viewModel = new LauncherViewModel(new FakeWinspotIpcClient(), hotkey);

        Assert.AreEqual("Ctrl Shift K", viewModel.HotkeyHint);
    }

    [TestMethod]
    public void UpdateHotkeyRegistrationStatus_ReportsUnavailableChord()
    {
        var viewModel = new LauncherViewModel(new FakeWinspotIpcClient());

        viewModel.UpdateHotkeyRegistrationStatus(false);

        Assert.IsTrue(viewModel.HotkeyStatusText.Contains("unavailable"));
    }

    [TestMethod]
    public void Query_WhenSetToText_ExpandsLauncherImmediately()
    {
        var viewModel = new LauncherViewModel(new FakeWinspotIpcClient());
        var raisedIsExpanded = false;
        viewModel.PropertyChanged += (_, args) =>
        {
            if (args.PropertyName == nameof(LauncherViewModel.IsExpanded))
            {
                raisedIsExpanded = true;
            }
        };

        viewModel.Query = "calc";

        Assert.IsTrue(viewModel.IsExpanded);
        Assert.IsTrue(raisedIsExpanded);
    }

    [TestMethod]
    public async Task RefreshAsync_PreservesSelectedResultByIdAcrossStreamedBatches()
    {
        var client = new FakeWinspotIpcClient();
        client.SearchBatches.Enqueue(new[]
        {
            Result("app:notes", "Notes"),
            Result("app:notepad", "Notepad"),
        });
        var viewModel = new LauncherViewModel(client);

        var firstSearch = client.WaitForSearchAsync();
        viewModel.Query = "note";
        await firstSearch;
        viewModel.SelectedResult = viewModel.Results[1];

        client.SearchBatches.Enqueue(new[]
        {
            Result("app:notepad", "Notepad"),
            Result("app:notes", "Notes"),
        });
        var secondSearch = client.WaitForSearchAsync();
        viewModel.Query = "not";
        await secondSearch;

        Assert.AreEqual("app:notepad", viewModel.SelectedResult?.Id);
    }

    [TestMethod]
    public void MoveSelectionDownAndUp_ChangesSelectedResult()
    {
        var viewModel = new LauncherViewModel(new FakeWinspotIpcClient());
        viewModel.Results.Add(Result("one", "One"));
        viewModel.Results.Add(Result("two", "Two"));
        viewModel.SelectedResult = viewModel.Results[0];

        viewModel.MoveSelectionDown();
        Assert.AreEqual("two", viewModel.SelectedResult?.Id);

        viewModel.MoveSelectionUp();
        Assert.AreEqual("one", viewModel.SelectedResult?.Id);
    }

    [TestMethod]
    public void FocusActions_UpdatesFocusedActionStateForActionStrip()
    {
        var viewModel = new LauncherViewModel(new FakeWinspotIpcClient());
        viewModel.SelectedResult = new SearchResultItem(
            "one",
            "One",
            "Subtitle",
            "App",
            1,
            "Open",
            new[]
            {
                new ActionItem("open", "Open", "Open"),
                new ActionItem("copy", "Copy", "Copy"),
            });

        Assert.AreEqual(0, viewModel.FocusedActionIndex);
        Assert.IsTrue(viewModel.SelectedActions[0].IsFocused);
        Assert.IsFalse(viewModel.SelectedActions[1].IsFocused);

        viewModel.FocusResults();

        Assert.AreEqual(-1, viewModel.FocusedActionIndex);
        Assert.IsFalse(viewModel.SelectedActions.Any(action => action.IsFocused));

        viewModel.FocusActions();

        Assert.AreEqual(0, viewModel.FocusedActionIndex);
        Assert.IsTrue(viewModel.SelectedActions[0].IsFocused);
    }

    [TestMethod]
    public void MoveFocusedActionRightAndLeft_ChangesFocusedAction()
    {
        var viewModel = new LauncherViewModel(new FakeWinspotIpcClient());
        viewModel.SelectedResult = new SearchResultItem(
            "one",
            "One",
            "Subtitle",
            "File",
            1,
            "Open",
            new[]
            {
                new ActionItem("open", "Open", "Open"),
                new ActionItem("copy-path", "Copy path", "CopyPath"),
            });

        viewModel.MoveActionRight();

        Assert.AreEqual(1, viewModel.FocusedActionIndex);
        Assert.IsTrue(viewModel.SelectedActions[1].IsFocused);

        viewModel.MoveActionLeft();

        Assert.AreEqual(0, viewModel.FocusedActionIndex);
        Assert.IsTrue(viewModel.SelectedActions[0].IsFocused);
    }

    [TestMethod]
    public async Task AcceptSelection_ReturnsTrueAfterSuccessfulAction()
    {
        var viewModel = new LauncherViewModel(new FakeWinspotIpcClient());
        viewModel.Results.Add(Result("one", "One"));
        viewModel.SelectedResult = viewModel.Results[0];

        var shouldHide = await viewModel.AcceptSelectionAsync();

        Assert.IsTrue(shouldHide);
        Assert.AreEqual("Executed", viewModel.StatusText);
    }

    [TestMethod]
    public async Task AcceptSelection_WhenActionChipIsFocused_ExecutesFocusedActionKind()
    {
        var client = new FakeWinspotIpcClient();
        var viewModel = new LauncherViewModel(client);
        viewModel.SelectedResult = new SearchResultItem(
            "file:C:\\Docs\\notes.txt",
            "notes.txt",
            "Subtitle",
            "File",
            1,
            "Open",
            new[]
            {
                new ActionItem("open", "Open", "Open"),
                new ActionItem("copy-path", "Copy path", "CopyPath"),
            });

        viewModel.MoveActionRight();
        await viewModel.AcceptSelectionAsync();

        Assert.AreEqual("CopyPath", client.LastActionKind);
    }

    [TestMethod]
    public async Task AcceptSelection_WhenActionsAreNotFocused_ExecutesPrimaryActionKind()
    {
        var client = new FakeWinspotIpcClient();
        var viewModel = new LauncherViewModel(client);
        viewModel.SelectedResult = new SearchResultItem(
            "file:C:\\Docs\\notes.txt",
            "notes.txt",
            "Subtitle",
            "File",
            1,
            "Open",
            new[]
            {
                new ActionItem("copy-path", "Copy path", "CopyPath"),
            });

        viewModel.FocusResults();
        await viewModel.AcceptSelectionAsync();

        Assert.AreEqual("Open", client.LastActionKind);
    }

    [TestMethod]
    public async Task AcceptSelection_WithSettingsCommand_RaisesSettingsRequestedWithoutDispatching()
    {
        var client = new FakeWinspotIpcClient();
        var viewModel = new LauncherViewModel(client);
        viewModel.Results.Add(Result(LauncherViewModel.SettingsCommandId, "Winspot Settings"));
        viewModel.SelectedResult = viewModel.Results[0];

        var settingsRequested = false;
        viewModel.SettingsRequested += (_, _) => settingsRequested = true;

        var shouldHide = await viewModel.AcceptSelectionAsync();

        Assert.IsTrue(shouldHide);
        Assert.IsTrue(settingsRequested, "Settings command should request the settings window.");
        Assert.IsNull(client.LastActionKind, "Settings command must not be dispatched to the daemon.");
    }

    private static SearchResultItem Result(string id, string title) => new(
        id,
        title,
        "Subtitle",
        "App",
        1,
        "Open");

    private sealed class FakeWinspotIpcClient : IWinspotIpcClient
    {
        private readonly Queue<TaskCompletionSource> _searchWaiters = new();

        public Queue<IReadOnlyList<SearchResultItem>> SearchBatches { get; } = new();

        public string? LastActionKind { get; private set; }

        public Task<string> ExecuteAsync(
            SearchResultItem result,
            ActionItem action,
            CancellationToken cancellationToken)
        {
            LastActionKind = action.Kind;
            return Task.FromResult("Executed");
        }

        public Task<PreviewItem?> GetPreviewAsync(SearchResultItem result, CancellationToken cancellationToken)
        {
            return Task.FromResult<PreviewItem?>(null);
        }

        public Task<PluginValidationReport> GetPluginDiagnosticsAsync(CancellationToken cancellationToken) =>
            Task.FromResult(PluginValidationReport.Empty);

        public async IAsyncEnumerable<IReadOnlyList<SearchResultItem>> StreamSearchAsync(
            string query,
            [EnumeratorCancellation] CancellationToken cancellationToken)
        {
            var batch = SearchBatches.Count == 0
                ? Array.Empty<SearchResultItem>()
                : SearchBatches.Dequeue();
            yield return batch;
            while (_searchWaiters.Count > 0)
            {
                _searchWaiters.Dequeue().SetResult();
            }
            await Task.CompletedTask;
        }

        public Task<IReadOnlyList<SearchResultItem>> SearchAsync(string query, CancellationToken cancellationToken)
        {
            return Task.FromResult<IReadOnlyList<SearchResultItem>>(Array.Empty<SearchResultItem>());
        }

        public Task WaitForSearchAsync()
        {
            var waiter = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
            _searchWaiters.Enqueue(waiter);
            return waiter.Task;
        }
    }
}
