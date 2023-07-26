import React from 'react';
import ReactDOM from 'react-dom/client';

import './index.css';
import UmiLogo from './assets/umi-logo.png';
import MailchimpUnsplash from './assets/mailchimp-unsplash.jpg';
import Person, { ReactComponent as PersonComponent } from './assets/person.svg';

function App() {
  return (
    <div>
      <PersonComponent width="40px" height="40px" />
      <img src={Person} />
      <div className="imageContainer">
        <img src={UmiLogo} />
      </div>
      <div>
        <img
          style={{ width: 200 }}
          src={MailchimpUnsplash}
          alt='unsplash big image'
        />
      </div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
