using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;

using Winspot_App.Models;
using Winspot_App.Services;

namespace Winspot_App.ViewModels;

public sealed class LauncherViewModel : INotifyPropertyChanged
{
    private readonly WinspotIpcClient _ipcClient;
    private CancellationTokenSource? _queryCancellation;
    private CancellationTokenSource? _actionCancellation;
    private CancellationTokenSource? _previewCancellation;
    private string _query = string.Empty;
    private PreviewItem? _preview;
    private SearchResultItem? _selectedResult;
    private string _statusText = "Start typing to search";

    public LauncherViewModel(WinspotIpcClient ipcClient)
    {
        _ipcClient = ipcClient;
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    public ObservableCollection<SearchResultItem> Results { get; } = new();

    public string Query
    {
        get => _query;
        set
        {
            if (SetField(ref _query, value))
            {
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
                _ = RefreshPreviewAsync(value);
            }
        }
    }

    public PreviewItem? Preview
    {
        get => _preview;
        private set => SetField(ref _preview, value);
    }

    public string StatusText
    {
        get => _statusText;
        private set => SetField(ref _statusText, value);
    }

    public async Task ExecuteSelectedAsync()
    {
        if (SelectedResult is null)
        {
            StatusText = "No result selected";
            return;
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
            }
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception ex)
        {
            StatusText = $"Action failed: {ex.Message}";
        }
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
            Preview = null;
            StatusText = "Start typing to search";
            return;
        }

        try
        {
            await Task.Delay(60, cancellationToken);
            StatusText = "Searching";

            var results = await _ipcClient.SearchAsync(query, cancellationToken);
            if (cancellationToken.IsCancellationRequested)
            {
                return;
            }

            Results.Clear();
            foreach (var result in results)
            {
                Results.Add(result);
            }

            SelectedResult = Results.FirstOrDefault();
            StatusText = Results.Count == 0 ? "No results" : $"{Results.Count} result(s)";
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception ex)
        {
            Results.Clear();
            SelectedResult = null;
            Preview = null;
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
            Preview = null;
            return;
        }

        Preview = new PreviewItem(result.Title, "Loading preview");

        try
        {
            var preview = await _ipcClient.GetPreviewAsync(result, cancellationToken);
            if (!cancellationToken.IsCancellationRequested)
            {
                Preview = preview ?? new PreviewItem(result.Title, "No preview available");
            }
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception ex)
        {
            Preview = new PreviewItem(result.Title, $"Preview unavailable: {ex.Message}");
        }
    }

    private bool SetField<T>(ref T field, T value, [CallerMemberName] string? propertyName = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value))
        {
            return false;
        }

        field = value;
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
        return true;
    }
}
