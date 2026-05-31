using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;

using Winspot_App.Models;
using Winspot_App.Services;

namespace Winspot_App.ViewModels;

public sealed class LauncherViewModel : INotifyPropertyChanged
{
    private readonly IWinspotIpcClient _ipcClient;
    private CancellationTokenSource? _queryCancellation;
    private CancellationTokenSource? _actionCancellation;
    private CancellationTokenSource? _previewCancellation;
    private string _query = string.Empty;
    private string _previewBody = "Select a result to preview";
    private string _previewTitle = "Preview";
    private SearchResultItem? _selectedResult;
    private string _statusText = "Start typing to search";
    private string _hotkeyHint;
    private string _hotkeyStatusText = "Hotkey not registered yet";
    private IReadOnlyList<ActionItem> _selectedResultActions = Array.Empty<ActionItem>();
    private IReadOnlyList<ActionViewItem> _selectedActions = Array.Empty<ActionViewItem>();
    private int _focusedActionIndex = -1;

    public LauncherViewModel(IWinspotIpcClient ipcClient, HotkeyBinding? hotkey = null)
    {
        _ipcClient = ipcClient;
        _hotkeyHint = (hotkey ?? new HotkeyBinding()).ToDisplayString();
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    public ObservableCollection<SearchResultItem> Results { get; } = new();

    /// The configured activation chord (e.g. "Ctrl Alt Space"), shown in the
    /// search box hint so it always reflects the user's actual hotkey.
    public string HotkeyHint
    {
        get => _hotkeyHint;
        private set => SetField(ref _hotkeyHint, value);
    }

    public string HotkeyStatusText
    {
        get => _hotkeyStatusText;
        private set => SetField(ref _hotkeyStatusText, value);
    }

    /// Refreshes the displayed chord after the user changes the hotkey in settings.
    public void UpdateHotkeyHint(HotkeyBinding hotkey)
    {
        HotkeyHint = hotkey.ToDisplayString();
    }

    public void UpdateHotkeyRegistrationStatus(bool registered)
    {
        HotkeyStatusText = registered
            ? $"Hotkey ready: {HotkeyHint}"
            : $"Hotkey unavailable: {HotkeyHint}";
    }

    public bool IsExpanded => !string.IsNullOrWhiteSpace(Query);

    public string Query
    {
        get => _query;
        set
        {
            if (SetField(ref _query, value))
            {
                OnPropertyChanged(nameof(IsExpanded));
                _ = RefreshAsync(value);
            }
        }
    }

    public SearchResultItem? SelectedResult
    {
        get => _selectedResult;
        set
        {
            if (SetField(ref _selectedResult, value))
            {
                _selectedResultActions = value?.DisplayActions ?? Array.Empty<ActionItem>();
                FocusedActionIndex = _selectedResultActions.Count > 0 ? 0 : -1;
                RefreshSelectedActions();
                _ = RefreshPreviewAsync(value);
            }
        }
    }

    public IReadOnlyList<ActionViewItem> SelectedActions
    {
        get => _selectedActions;
        private set => SetField(ref _selectedActions, value);
    }

    public int FocusedActionIndex
    {
        get => _focusedActionIndex;
        private set
        {
            if (SetField(ref _focusedActionIndex, value))
            {
                RefreshSelectedActions();
            }
        }
    }

    public string PreviewBody
    {
        get => _previewBody;
        private set => SetField(ref _previewBody, value);
    }

    public string PreviewTitle
    {
        get => _previewTitle;
        private set => SetField(ref _previewTitle, value);
    }

    public string StatusText
    {
        get => _statusText;
        private set => SetField(ref _statusText, value);
    }

    public Task ExecuteSelectedAsync() => AcceptSelectionAsync();

    public async Task<bool> AcceptSelectionAsync()
    {
        if (SelectedResult is null)
        {
            StatusText = "No result selected";
            return false;
        }

        _actionCancellation?.Cancel();
        _actionCancellation = new CancellationTokenSource();
        var cancellationToken = _actionCancellation.Token;

        try
        {
            StatusText = $"Running {SelectedResult.Title}";
            var message = await _ipcClient.ExecuteAsync(SelectedResult, cancellationToken);
            if (!cancellationToken.IsCancellationRequested)
            {
                StatusText = message;
                return true;
            }
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception ex)
        {
            StatusText = $"Action failed: {ex.Message}";
        }

        return false;
    }

    public void MoveSelectionDown()
    {
        MoveSelection(1);
    }

    public void MoveSelectionUp()
    {
        MoveSelection(-1);
    }

    public void FocusActions()
    {
        FocusedActionIndex = _selectedResultActions.Count > 0 ? 0 : -1;
    }

    public void FocusResults()
    {
        FocusedActionIndex = -1;
    }

    private void MoveSelection(int delta)
    {
        if (Results.Count == 0)
        {
            SelectedResult = null;
            return;
        }

        var currentIndex = SelectedResult is null ? -1 : Results.IndexOf(SelectedResult);
        var next = currentIndex < 0
            ? 0
            : Math.Clamp(currentIndex + delta, 0, Results.Count - 1);
        SelectedResult = Results[next];
    }

    private void RefreshSelectedActions()
    {
        SelectedActions = _selectedResultActions
            .Select((action, index) => new ActionViewItem(
                action.Id,
                action.Label,
                index == FocusedActionIndex))
            .ToArray();
    }

    private async Task RefreshAsync(string query)
    {
        _queryCancellation?.Cancel();
        _queryCancellation = new CancellationTokenSource();
        var cancellationToken = _queryCancellation.Token;

        if (string.IsNullOrWhiteSpace(query))
        {
            Results.Clear();
            SelectedResult = null;
            ResetPreview();
            StatusText = "Start typing to search";
            return;
        }

        try
        {
            await Task.Delay(60, cancellationToken);
            StatusText = "Searching";

            await foreach (var results in _ipcClient.StreamSearchAsync(query, cancellationToken))
            {
                if (cancellationToken.IsCancellationRequested)
                {
                    return;
                }

                var selectedId = SelectedResult?.Id;
                Results.Clear();
                foreach (var result in results)
                {
                    Results.Add(result);
                }

                SelectedResult = selectedId is null
                    ? Results.FirstOrDefault()
                    : Results.FirstOrDefault(result => result.Id == selectedId) ?? Results.FirstOrDefault();
                StatusText = Results.Count == 0 ? "No results" : $"{Results.Count} result(s)";
            }
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception ex)
        {
            Results.Clear();
            SelectedResult = null;
            ResetPreview();
            StatusText = $"Backend unavailable: {ex.Message}";
        }
    }

    private async Task RefreshPreviewAsync(SearchResultItem? result)
    {
        _previewCancellation?.Cancel();
        _previewCancellation = new CancellationTokenSource();
        var cancellationToken = _previewCancellation.Token;

        if (result is null)
        {
            ResetPreview();
            return;
        }

        SetPreview(result.Title, "Loading preview");

        try
        {
            var preview = await _ipcClient.GetPreviewAsync(result, cancellationToken);
            if (!cancellationToken.IsCancellationRequested)
            {
                SetPreview(
                    preview?.Title ?? result.Title,
                    preview?.Body ?? "No preview available");
            }
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception ex)
        {
            SetPreview(result.Title, $"Preview unavailable: {ex.Message}");
        }
    }

    private void ResetPreview()
    {
        SetPreview("Preview", "Select a result to preview");
    }

    private void SetPreview(string title, string body)
    {
        PreviewTitle = title;
        PreviewBody = body;
    }

    private bool SetField<T>(ref T field, T value, [CallerMemberName] string? propertyName = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value))
        {
            return false;
        }

        field = value;
            OnPropertyChanged(propertyName);
        return true;
    }

    private void OnPropertyChanged(string? propertyName)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
    }
}
