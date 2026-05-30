using System.Collections.Generic;

using Microsoft.VisualStudio.TestTools.UnitTesting;

using Winspot_App.Models;

namespace Winspot_App.Tests.Models;

[TestClass]
public sealed class HotkeyBindingTests
{
    [TestMethod]
    public void ToDisplayString_Default_IsCtrlAltSpace()
    {
        Assert.AreEqual("Ctrl Alt Space", new HotkeyBinding().ToDisplayString());
    }

    [TestMethod]
    public void ToDisplayString_NormalizesModifierAliasesAndCasing()
    {
        var binding = new HotkeyBinding
        {
            Key = "space",
            Modifiers = new List<string> { "control", "WIN" },
        };

        Assert.AreEqual("Ctrl Win Space", binding.ToDisplayString());
    }

    [TestMethod]
    public void ToDisplayString_UppercasesSingleCharacterKey()
    {
        var binding = new HotkeyBinding
        {
            Key = "k",
            Modifiers = new List<string> { "Alt" },
        };

        Assert.AreEqual("Alt K", binding.ToDisplayString());
    }
}
