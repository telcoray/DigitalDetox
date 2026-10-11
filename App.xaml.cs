using System.Windows;

namespace DigitalDetox
{
    public partial class App : Application
    {
        // Kept in a field so it stays alive (and held) for the whole lifetime of the app
        private System.Threading.Mutex? _singleInstanceMutex;

        protected override void OnStartup(StartupEventArgs e)
        {
            base.OnStartup(e);

            // Ensure only one instance runs
            _singleInstanceMutex = new System.Threading.Mutex(true, "DigitalDetoxApp", out bool createdNew);
            if (!createdNew)
            {
                MessageBox.Show("Digital Detox is already running.", "Already Running", MessageBoxButton.OK, MessageBoxImage.Information);
                Shutdown();
                return;
            }
        }
    }
}