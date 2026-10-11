using System;
using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

namespace DigitalDetox
{
    public partial class MainWindow : Window
    {
        public MainWindow()
        {
            InitializeComponent();
        }

        private void NumberValidationTextBox(object sender, TextCompositionEventArgs e)
        {
            var regex = new Regex("[^0-9]+");
            e.Handled = regex.IsMatch(e.Text);
        }

        private void QuickTime_Click(object sender, RoutedEventArgs e)
        {
            if (sender is Button btn && btn.Tag is string tag && int.TryParse(tag, out int minutes))
            {
                MinutesTextBox.Text = minutes.ToString();
            }
        }

        private void StartButton_Click(object sender, RoutedEventArgs e)
        {
            if (!int.TryParse(MinutesTextBox.Text, out int minutes) || minutes <= 0)
            {
                MessageBox.Show("Please enter a valid number of minutes (1 or more).", "Invalid Input", MessageBoxButton.OK, MessageBoxImage.Warning);
                return;
            }

            if (minutes > 1440) // Max 24 hours
            {
                MessageBox.Show("Maximum duration is 24 hours (1440 minutes).", "Too Long", MessageBoxButton.OK, MessageBoxImage.Warning);
                return;
            }

            // Hide main window and show lock screen
            this.Hide();
            
            var lockWindow = new LockWindow(TimeSpan.FromMinutes(minutes));
            lockWindow.Closed += (s, args) => this.Show();
            lockWindow.Show();
        }
    }
}