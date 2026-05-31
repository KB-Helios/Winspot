using Microsoft.VisualStudio.TestTools.UnitTesting;

using Winspot_App.Models;

namespace Winspot_App.Tests.Models;

[TestClass]
public sealed class SearchResultItemTests
{
    [TestMethod]
    public void IconPath_ForApp_StripsPrefix()
    {
        var item = new SearchResultItem(
            @"app:C:\Apps\Editor.lnk",
            "Editor",
            "App",
            "App",
            1.0,
            "Open");

        Assert.AreEqual(@"C:\Apps\Editor.lnk", item.IconPath);
        Assert.IsTrue(item.HasIcon);
    }

    [TestMethod]
    public void IconPath_ForFileAndFolder_StripsPrefix()
    {
        var file = new SearchResultItem(@"file:C:\Docs\notes.txt", "notes.txt", "", "File", 1.0, "Open");
        var folder = new SearchResultItem(@"folder:C:\Docs", "Docs", "", "Folder", 1.0, "Open");

        Assert.AreEqual(@"C:\Docs\notes.txt", file.IconPath);
        Assert.AreEqual(@"C:\Docs", folder.IconPath);
    }

    [TestMethod]
    public void IconPath_ForNonPathKinds_IsNull()
    {
        var command = new SearchResultItem("command:calculator", "Calculator", "", "Command", 1.0, "RunCommand");
        var process = new SearchResultItem("process:1234", "winspot", "", "Process", 1.0, "Open");

        Assert.IsNull(command.IconPath);
        Assert.IsNull(process.IconPath);
        Assert.IsFalse(command.HasIcon);
    }
}
