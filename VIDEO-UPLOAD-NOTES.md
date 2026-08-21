# ghvid pipeline — two traps, both learned the hard way

## 1. Never hash a file while it is still downloading

`curl` returning is not the same as the bytes being on disk and settled. Hashing
a partially-written file gives a short size and a wrong md5, which then reads as
a *transfer mismatch* — a real-looking failure caused entirely by measuring too
early. This happened twice in one session and nearly caused a good clip to be
withdrawn.

```bash
curl -s -L --retry 3 "$URL" -o "$F"
a=0; b=1; while [ "$a" != "$b" ]; do a=$(stat -f%z "$F"); sleep 1; b=$(stat -f%z "$F"); done
md5 -q "$F"      # only now
```

## 2. The attachment size limit is not a constant — do not predict it

One upload was rejected with S3 `EntityTooLarge`:

```
ProposedSize 22670023   MaxSizeAllowed 22667264
```

It is tempting to treat 22,667,264 as the cap and pre-emptively re-encode
anything larger. **That is wrong**: a **26,082,003-byte** file uploaded
successfully a few minutes later. Whatever the limit depends on, it is not a
fixed byte count.

So: **upload first, and re-encode only if the upload actually fails.** A
prediction here costs quality on files that would have gone through untouched.
The full-quality original should always go to the release asset regardless, so
the re-encode only ever affects the inline player.

## The general shape

Both of these are the same mistake: **a check that was wrong about data that was
right.** Before concluding a file is bad, confirm the instrument was in a
position to measure it — settled bytes, a threshold with more than one
observation behind it, a tool that reports what it actually did rather than an
exit code you inferred meaning from.

## 3. The CSRF token is not a form input

This is what made every early attempt return 422. The token the drag-and-drop
upload needs lives in the **edit page's embedded JSON**, not in any `<input>`:

```
csrf_tokens['/upload/policies/assets'].post
```

Fetch any file's edit page (`https://github.com/<owner>/<repo>/edit/main/<path>`)
with the session cookie and read it from there. The three-request dance is then:

1. `POST /upload/policies/assets` → an S3 policy, an asset id, **and a second
   token** (the one step 3 needs — it is not the same token as step 1's)
2. `POST` the bytes to the S3 URL, using that policy's own form fields verbatim
3. `PUT /upload/assets/<id>` with the second token, to finalise

## 4. A fresh asset 404s publicly until a commit references it

Expected, not a failure. Verify with the session cookie
(`curl -sL -b "$(cat ~/.gh-upload/cookie)" <url>`) and check the md5 against what
you uploaded. It becomes public when a README referencing it lands.
