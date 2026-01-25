import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { SettingsWindow } from './windows/SettingsWindow'
import './index.css'

const urlParams = new URLSearchParams(window.location.search);
const windowType = urlParams.get('window');

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    {windowType === 'settings' ? <SettingsWindow /> : <App />}
  </React.StrictMode>,
)
