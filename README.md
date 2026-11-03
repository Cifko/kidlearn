# KidLearn (working title)

A learning app for little kids (elementary school, grades 1–4). Kids browse
learning subjects shared by other users, or describe what they want to learn
and get an AI-generated course: ordered lessons with small
Duolingo-style multiple-choice questions, with pictures.

## How it works

1. The user signs up / logs in (Google OAuth planned; a dev-login button
   exists for development).
2. They browse public subjects, or create their own (public or private) by
   describing the subject in their own language (e.g. a Slovak kid types
   "prvouka").
3. The backend asks the Meta Model API (`api.meta.ai`) for N lessons (default 20,
   configurable) in pedagogical order, then generates 5–10 questions per
   lesson. Questions reference image assets; pictures are generated on demand.
4. The kid opens a subject, sees the ordered lessons, and learns
   through them with Duolingo-like progress tracking.
5. Every AI call's prompt and answer is stored; open "AI prompts" from
   home to inspect what the model saw when an output looks wrong.

## Layout

- `backend/` — Rust (Axum) API: auth, subjects, Meta AI generation, SQLite.
- `app/` — Expo (React Native + Web) client from one codebase: web app and
  native mobile apps.
- `backend/prompts/` — prompt templates (planner, questions, pictures).

## Quickstart

Backend:

```sh
cd backend
cp .env.example .env   # set META_API_KEY to enable generation
cargo run
# API on http://localhost:3000, health at /health
```

App (web):

```sh
cd app
npm install
npx expo start --web
# open http://localhost:8081 in a browser
```

App (phone): install Expo Go, run `npx expo start`, scan the QR code.

## Environment

| Variable        | Default                              | Purpose                          |
| --------------- | ------------------------------------ | -------------------------------- |
| `DATABASE_URL`  | `sqlite://kidlearn.db`               | SQLite database                  |
| `JWT_SECRET`    | `dev-secret-change-me`               | Token signing (dev default)      |
| `META_API_BASE` | `https://api.meta.ai/v1`             | Meta Model API endpoint          |
| `META_API_KEY`  | _(none)_                             | Enables AI generation            |
| `META_MODEL`    | `muse-spark-1.3`                     | Generation model                 |
| `GOOGLE_CLIENT_ID` | _(none)_                          | Enables Google login (web client ID) |
| `FACEBOOK_APP_ID` / `FACEBOOK_APP_SECRET` | _(none)_       | Enables Facebook login             |
| `RUST_LOG` | `info` | Log verbosity (`debug` shows every HTTP request) |

## Google login setup

1. In [Google Cloud Console](https://console.cloud.google.com/), create OAuth
   credentials of type **Web application**.
2. Add the app's address to **Authorized redirect URIs**
   (e.g. `http://localhost:8081` for local web dev). The app builds its
   redirect with `AuthSession.makeRedirectUri()`.
3. Put the same client ID in `backend/.env` (`GOOGLE_CLIENT_ID=…`) and in
   `app/.env` (`EXPO_PUBLIC_GOOGLE_CLIENT_ID=…`), then restart both.
   With no client ID configured, the Google button stays disabled and the
   dev login keeps working. Native apps need a dev build plus their own
   iOS/Android client IDs (not wired yet).

## Facebook login setup

1. At [Meta for Developers](https://developers.facebook.com/), create an app
   (type **Consumer**) and add the **Facebook Login** product.
2. Under Facebook Login settings, add the app's address to **Valid OAuth
   Redirect URIs** (e.g. `http://localhost:8081` for local web dev, plus
   `http://localhost:8081/` with trailing slash — Facebook matches
   exactly). The app builds its redirect with
   `AuthSession.makeRedirectUri()`.
3. In **App settings → Basic**, copy the App ID and App secret.
4. Put them in `backend/.env` (`FACEBOOK_APP_ID=…`,
   `FACEBOOK_APP_SECRET=…`) and the App ID in `app/.env`
   (`EXPO_PUBLIC_FACEBOOK_APP_ID=…`), then restart both.
   With nothing configured, the Facebook button stays disabled and the
   dev login keeps working. Like Google, phone login needs a dev build
   (OAuth redirects don't work in Expo Go).

## Roadmap

- Durable job queue for generation (in-process tasks are lost on restart)
- More UI languages (English + Slovak first)
