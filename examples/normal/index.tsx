import React from 'react';
import ReactDOM from 'react-dom/client';

import MailchimpUnsplash from './assets/mailchimp-unsplash.jpg';
import Person, { ReactComponent as PersonComponent } from './assets/person.svg';
import UmiLogo from './assets/umi-logo.png';
import { foo } from './foo';
import './index.css';
import { Test } from './app';
import styles from './style.module.css';

function App() {
  return (
    <div>
      <Test></Test>
      <h1 className={styles.title}>Hello {foo}!</h1>
      <PersonComponent width="40px" height="40px" />
      <img src={Person} />
      <div className="imageContainer">
        <img className={styles.image} src={UmiLogo} />
      </div>
      <div>
        <img
          style={{ width: 200 }}
          src={MailchimpUnsplash}
          alt="unsplash big image"
        />
      </div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
