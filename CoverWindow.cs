using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace DigitalDetox
{
    /// <summary>
    /// Full-screen overlay for a secondary monitor while the detox is running.
    /// The main LockWindow owns these and closes them when the detox ends.
    /// </summary>
    public class CoverWindow : Window
    {
        private readonly TextBlock _timerText;
        private bool _allowClose = false;

        public CoverWindow(Rect bounds)
        {
            WindowStyle = WindowStyle.None;
            ResizeMode = ResizeMode.NoResize;
            Topmost = true;
            ShowInTaskbar = false;
            ShowActivated = false;
            Background = new SolidColorBrush(Color.FromRgb(0x1A, 0x1A, 0x2E));

            // Place the window on its monitor first, then maximize so it covers that monitor (taskbar included)
            WindowStartupLocation = WindowStartupLocation.Manual;
            Left = bounds.Left;
            Top = bounds.Top;
            Width = bounds.Width;
            Height = bounds.Height;
            Loaded += (s, e) => WindowState = WindowState.Maximized;

            _timerText = new TextBlock
            {
                FontFamily = new FontFamily("Segoe UI"),
                FontSize = 72,
                FontWeight = FontWeights.Thin,
                Foreground = Brushes.White,
                HorizontalAlignment = HorizontalAlignment.Center
            };

            Content = new StackPanel
            {
                VerticalAlignment = VerticalAlignment.Center,
                Children =
                {
                    new TextBlock
                    {
                        Text = "🌿 Time to Disconnect",
                        FontFamily = new FontFamily("Segoe UI"),
                        FontSize = 28,
                        FontWeight = FontWeights.Bold,
                        Foreground = new SolidColorBrush(Color.FromRgb(0xEC, 0xF0, 0xF1)),
                        HorizontalAlignment = HorizontalAlignment.Center,
                        Margin = new Thickness(0, 0, 0, 20)
                    },
                    _timerText
                }
            };

            Closing += (s, e) => { if (!_allowClose) e.Cancel = true; };
        }

        public void SetTimerText(string text) => _timerText.Text = text;

        public void ForceClose()
        {
            _allowClose = true;
            Close();
        }
    }
}
