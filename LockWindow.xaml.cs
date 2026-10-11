using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Threading;

namespace DigitalDetox
{
    public partial class LockWindow : Window
    {
        private readonly DispatcherTimer _timer;
        private readonly TimeSpan _totalDuration;
        private TimeSpan _remainingTime;
        private readonly DateTime _endTime;
        private readonly DateTime _endTimeUtc;
        private bool _allowClose = false;
        private int _emergencyKeyPressCount = 0;
        private DateTime _lastEmergencyKeyPress = DateTime.MinValue;

        // Windows API for low-level keyboard hook
        private const int WH_KEYBOARD_LL = 13;
        private const int WM_KEYDOWN = 0x0100;
        private const int WM_SYSKEYDOWN = 0x0104;
        private const int VK_LWIN = 0x5B;
        private const int VK_RWIN = 0x5C;
        private const int VK_TAB = 0x09;
        private const int VK_ESCAPE = 0x1B;
        private const int VK_F4 = 0x73;
        private const int VK_MENU = 0x12;
        private const int VK_CONTROL = 0x11;

        private delegate IntPtr LowLevelKeyboardProc(int nCode, IntPtr wParam, IntPtr lParam);
        private static LowLevelKeyboardProc _keyboardProc = HookCallback;
        private static IntPtr _hookID = IntPtr.Zero;

        [DllImport("user32.dll", CharSet = CharSet.Auto, SetLastError = true)]
        private static extern IntPtr SetWindowsHookEx(int idHook, LowLevelKeyboardProc lpfn, IntPtr hInstance, uint threadId);

        [DllImport("user32.dll", CharSet = CharSet.Auto, SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool UnhookWindowsHookEx(IntPtr hhk);

        [DllImport("user32.dll", CharSet = CharSet.Auto, SetLastError = true)]
        private static extern IntPtr CallNextHookEx(IntPtr hhk, int nCode, IntPtr wParam, IntPtr lParam);

        [DllImport("kernel32.dll", CharSet = CharSet.Auto, SetLastError = true)]
        private static extern IntPtr GetModuleHandle(string? lpModuleName);

        [DllImport("user32.dll")]
        private static extern short GetAsyncKeyState(int vKey);

        // Windows API for enumerating monitors
        private const uint MONITORINFOF_PRIMARY = 1;

        [StructLayout(LayoutKind.Sequential)]
        private struct RECT { public int Left, Top, Right, Bottom; }

        [StructLayout(LayoutKind.Sequential)]
        private struct MONITORINFO
        {
            public int cbSize;
            public RECT rcMonitor;
            public RECT rcWork;
            public uint dwFlags;
        }

        private delegate bool MonitorEnumProc(IntPtr hMonitor, IntPtr hdcMonitor, ref RECT lprcMonitor, IntPtr dwData);

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool EnumDisplayMonitors(IntPtr hdc, IntPtr lprcClip, MonitorEnumProc lpfnEnum, IntPtr dwData);

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool GetMonitorInfo(IntPtr hMonitor, ref MONITORINFO lpmi);

        private readonly List<CoverWindow> _coverWindows = new();

        public LockWindow(TimeSpan duration)
        {
            InitializeComponent();
            
            _totalDuration = duration;
            _remainingTime = duration;
            _endTime = DateTime.Now.Add(duration);
            _endTimeUtc = DateTime.UtcNow.Add(duration);

            // Tick faster than once per second: remaining time is computed from the clock,
            // so a short interval just keeps the display from skipping seconds.
            _timer = new DispatcherTimer
            {
                Interval = TimeSpan.FromMilliseconds(250)
            };
            _timer.Tick += Timer_Tick;

            UpdateTimerDisplay();
            UpdateEndTimeDisplay();
            SetRandomMotivation();
        }

        private void Window_Loaded(object sender, RoutedEventArgs e)
        {
            _hookID = SetHook(_keyboardProc);
            this.Topmost = true;
            this.Activate();
            this.Focus();
            CoverSecondaryMonitors();
            _timer.Start();
        }

        private void CoverSecondaryMonitors()
        {
            // Monitor bounds come in physical pixels; WPF positions windows in device-independent units
            var source = PresentationSource.FromVisual(this);
            if (source?.CompositionTarget == null) return;
            var fromDevice = source.CompositionTarget.TransformFromDevice;

            var secondaryBounds = new List<Rect>();
            EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, (IntPtr hMonitor, IntPtr hdc, ref RECT rect, IntPtr data) =>
            {
                var info = new MONITORINFO { cbSize = Marshal.SizeOf<MONITORINFO>() };
                if (GetMonitorInfo(hMonitor, ref info) && (info.dwFlags & MONITORINFOF_PRIMARY) == 0)
                {
                    var r = info.rcMonitor;
                    secondaryBounds.Add(new Rect(
                        fromDevice.Transform(new Point(r.Left, r.Top)),
                        fromDevice.Transform(new Point(r.Right, r.Bottom))));
                }
                return true;
            }, IntPtr.Zero);

            foreach (var bounds in secondaryBounds)
            {
                var cover = new CoverWindow(bounds);
                cover.SetTimerText(TimerText.Text);
                cover.Show();
                _coverWindows.Add(cover);
            }
        }

        private void Window_Deactivated(object sender, EventArgs e)
        {
            if (!_allowClose)
            {
                this.Dispatcher.BeginInvoke(new Action(() => { this.Topmost = true; this.Activate(); this.Focus(); }), DispatcherPriority.Send);
            }
        }

        private void Window_Closing(object sender, System.ComponentModel.CancelEventArgs e)
        {
            if (!_allowClose) e.Cancel = true;
            else Cleanup();
        }

        private void Timer_Tick(object? sender, EventArgs e)
        {
            _remainingTime = _endTimeUtc - DateTime.UtcNow;
            if (_remainingTime < TimeSpan.Zero) _remainingTime = TimeSpan.Zero;
            UpdateTimerDisplay();
            UpdateProgress();
            if (_remainingTime <= TimeSpan.Zero) { _timer.Stop(); CompleteDetox(); }
        }

        private void UpdateTimerDisplay()
        {
            // Round up so the display only reaches 00:00 when the detox actually ends
            var shown = TimeSpan.FromSeconds(Math.Ceiling(_remainingTime.TotalSeconds));
            TimerText.Text = shown.TotalHours >= 1
                ? $"{(int)shown.TotalHours}:{shown:mm\\:ss}"
                : shown.ToString(@"mm\:ss");
            foreach (var cover in _coverWindows) cover.SetTimerText(TimerText.Text);
        }

        private void UpdateProgress()
        {
            double progress = Math.Max(0, Math.Min(1, 1.0 - (_remainingTime.TotalSeconds / _totalDuration.TotalSeconds)));
            ProgressFill.Width = 400 * progress;
            ProgressText.Text = $"{(int)(progress * 100)}% complete";
        }

        private void UpdateEndTimeDisplay() => EndTimeText.Text = $"Unlocks at: {_endTime:T}"; // System time format: 19:19:33 (it-IT), 7:19:33 PM (en-US)

        private void SetRandomMotivation()
        {
            var messages = new[] { "Take a deep breath. Relax your shoulders.", "Your mind deserves this break.", "Disconnect to reconnect with yourself.", "Breathe in. Breathe out. Just be.", "No notifications. No pressure. Just peace.", "This time is yours alone.", "Let your eyes rest. Let your mind wander.", "You're doing great. Keep going.", "The world can wait. You matter.", "Find calm in the stillness." };
            MotivationText.Text = messages[new Random().Next(messages.Length)];
        }

        private void CompleteDetox()
        {
            StatusText.Text = "✨ Detox Complete!"; StatusText.Foreground = Brushes.LightGreen;
            TimerText.Text = "00:00"; TimerText.Foreground = Brushes.LightGreen;
            ProgressFill.Background = Brushes.LightGreen; ProgressText.Text = "100% complete";
            System.Media.SystemSounds.Asterisk.Play();
            var closeTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(3) };
            closeTimer.Tick += (s, e) => { closeTimer.Stop(); _allowClose = true; this.Close(); };
            closeTimer.Start();
        }

        private void Window_KeyDown(object sender, KeyEventArgs e) => HandleKeyCombination(e);

        private void Window_PreviewKeyDown(object sender, KeyEventArgs e) { if (IsBlockedKey(e.Key)) e.Handled = true; }

        private bool IsBlockedKey(Key key)
        {
            if (key == Key.LWin || key == Key.RWin) return true;
            if (Keyboard.Modifiers.HasFlag(ModifierKeys.Alt) && (key == Key.Tab || key == Key.F4 || key == Key.Escape)) return true;
            // Also covers Ctrl+Shift+Esc (Task Manager)
            if (Keyboard.Modifiers.HasFlag(ModifierKeys.Control) && key == Key.Escape) return true;
            return false;
        }

        private void HandleKeyCombination(KeyEventArgs e)
        {
            if (Keyboard.Modifiers.HasFlag(ModifierKeys.Control) && Keyboard.Modifiers.HasFlag(ModifierKeys.Shift) && e.Key == Key.X)
            {
                var now = DateTime.Now;
                _emergencyKeyPressCount = (now - _lastEmergencyKeyPress).TotalSeconds < 2 ? _emergencyKeyPressCount + 1 : 1;
                _lastEmergencyKeyPress = now;
                if (_emergencyKeyPressCount >= 3) { EmergencyExitButton.Visibility = Visibility.Visible; _emergencyKeyPressCount = 0; }
                else { StatusText.Text = $"Emergency exit: Press Ctrl+Shift+X {3 - _emergencyKeyPressCount} more time(s)"; StatusText.Foreground = Brushes.Orange; }
                e.Handled = true;
            }
        }

        private void EmergencyExitButton_Click(object sender, RoutedEventArgs e)
        {
            var result = MessageBox.Show("Are you sure you want to end your detox early?\n\nYour progress will be lost.", "Early Exit", MessageBoxButton.YesNo, MessageBoxImage.Warning, MessageBoxResult.No);
            if (result == MessageBoxResult.Yes) { _allowClose = true; this.Close(); }
            else { EmergencyExitButton.Visibility = Visibility.Collapsed; StatusText.Text = "Digital Detox in Progress"; StatusText.Foreground = Brushes.Gray; }
        }

        private static IntPtr SetHook(LowLevelKeyboardProc proc)
        {
            using var curProcess = Process.GetCurrentProcess();
            using var curModule = curProcess.MainModule;
            return SetWindowsHookEx(WH_KEYBOARD_LL, proc, GetModuleHandle(curModule?.ModuleName), 0);
        }

        private static IntPtr HookCallback(int nCode, IntPtr wParam, IntPtr lParam)
        {
            if (nCode >= 0 && (wParam == (IntPtr)WM_KEYDOWN || wParam == (IntPtr)WM_SYSKEYDOWN))
            {
                int vkCode = Marshal.ReadInt32(lParam);
                if (vkCode == VK_LWIN || vkCode == VK_RWIN) return (IntPtr)1;
                if (vkCode == VK_TAB && (GetAsyncKeyState(VK_MENU) & 0x8000) != 0) return (IntPtr)1;
                if (vkCode == VK_F4 && (GetAsyncKeyState(VK_MENU) & 0x8000) != 0) return (IntPtr)1;
                // Also covers Ctrl+Shift+Esc (Task Manager). Ctrl+Alt+Del can't be intercepted by applications.
                if (vkCode == VK_ESCAPE && (GetAsyncKeyState(VK_CONTROL) & 0x8000) != 0) return (IntPtr)1;
            }
            return CallNextHookEx(_hookID, nCode, wParam, lParam);
        }

        private void Cleanup()
        {
            _timer.Stop();
            foreach (var cover in _coverWindows) cover.ForceClose();
            _coverWindows.Clear();
            if (_hookID != IntPtr.Zero) { UnhookWindowsHookEx(_hookID); _hookID = IntPtr.Zero; }
        }
    }
}