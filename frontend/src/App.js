import React, { useState, useEffect } from 'react';
import logo from './logo.svg';
import './App.css';

function App() {
  const [apiResponse, setApiResponse] = useState('');

  useEffect(() => {
    fetch('/api/test') // Use relative path
      .then(response => response.text())
      .then(data => setApiResponse(data))
      .catch(error => console.error('Error fetching data:', error));
  }, []);

  return (
    <div className="App">
      <header className="App-header">
        <img src={logo} className="App-logo" alt="logo" />
        <p>
          Edit <code>src/App.js</code> and save to reload. Hello?
        </p>
        <a
          className="App-link"
          href="https://reactjs.org"
          target="_blank"
          rel="noopener noreferrer"
        >
          Learn React
        </a>
        <div>
          <textarea
            value={apiResponse}
            readOnly
            rows="4"
            cols="50"
          />
        </div>
      </header>
    </div>
  );
}

export default App;
