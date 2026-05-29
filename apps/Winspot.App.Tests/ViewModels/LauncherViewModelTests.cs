using System;
using System.Collections.Generic;
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

    private sealed class FakeWinspotIpcClient : IWinspotIpcClient
    {
        public Task<string> ExecuteAsync(SearchResultItem result, CancellationToken cancellationToken)
        {
            return Task.FromResult("Executed");
        }

        public Task<PreviewItem?> GetPreviewAsync(SearchResultItem result, CancellationToken cancellationToken)
        {
            return Task.FromResult<PreviewItem?>(null);
        }

        public Task<IReadOnlyList<SearchResultItem>> SearchAsync(string query, CancellationToken cancellationToken)
        {
            return Task.FromResult<IReadOnlyList<SearchResultItem>>(Array.Empty<SearchResultItem>());
        }
    }
}
