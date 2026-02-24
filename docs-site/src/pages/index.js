import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';

import Heading from '@theme/Heading';
import styles from './index.module.css';

function HomepageHeader() {
  const { siteConfig } = useDocusaurusContext();
  return (
    <header className={clsx('hero hero--primary', styles.heroBanner)}>
      <div className="container">
        <Heading as="h1" className="hero__title">
          🔒 {siteConfig.title}
        </Heading>
        <p className="hero__subtitle">{siteConfig.tagline}</p>
        <p style={{ fontSize: '1.1rem', opacity: 0.9, maxWidth: 600, margin: '0 auto 1.5rem' }}>
          Prevent the Multi-Agent Race Condition (MARC) — the silent data loss
          when autonomous AI agents simultaneously modify shared resources.
        </p>
        <div className={styles.buttons}>
          <Link
            className="button button--secondary button--lg"
            to="/docs/getting-started">
            Get Started → 5min ⏱️
          </Link>
        </div>
      </div>
    </header>
  );
}

function Feature({ title, description }) {
  return (
    <div className={clsx('col col--4')}>
      <div className="text--center padding-horiz--md padding-vert--lg">
        <Heading as="h3">{title}</Heading>
        <p>{description}</p>
      </div>
    </div>
  );
}

function HomepageFeatures() {
  return (
    <section style={{ padding: '2rem 0' }}>
      <div className="container">
        <div className="row">
          <Feature
            title="⚡ Sub-Nanosecond Conflict Detection"
            description="O(1) conflict checks via a precomputed 6×6 compatibility matrix. No locks, no GC, pure Rust."
          />
          <Feature
            title="🛡️ Deadlock-Free Scheduling"
            description="Wait-Die protocol guarantees no circular waits. The oldest agent always makes progress."
          />
          <Feature
            title="🌍 Multi-Language SDKs"
            description="Native bindings for Python (PyO3) and JavaScript (napi-rs). pip install or npm install."
          />
        </div>
      </div>
    </section>
  );
}

export default function Home() {
  const { siteConfig } = useDocusaurusContext();
  return (
    <Layout
      title="Concurrency Control for AI Agents"
      description="Klock prevents multi-agent race conditions with O(1) conflict detection and Wait-Die scheduling.">
      <HomepageHeader />
      <main>
        <HomepageFeatures />
      </main>
    </Layout>
  );
}
