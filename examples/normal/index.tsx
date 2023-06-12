import React from 'react';
import ReactDOM from 'react-dom/client';

import { foo } from './foo';
import './index.css';
import styles from './style.module.css';
import UmiLogo from './assets/umi-logo.png';
import MailchimpUnsplash from './assets/mailchimp-unsplash.jpg';

function App() {
  return (
    <div>
      <h1 className={styles.title}>Hello {foo}!</h1>
      <div className="imageContainer">
        <img className={styles.image} src={UmiLogo} />
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
