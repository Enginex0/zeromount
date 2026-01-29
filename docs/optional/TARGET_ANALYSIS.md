# Target Analysis

> **Purpose:** Document reverse engineering findings for LSposed/hook projects.
> **Use:** Copy to docs/ when doing reverse engineering work.
> **Update:** As you discover more about the target.

---

## Target Application

| Property | Value |
|----------|-------|
| **App Name** | |
| **Package** | |
| **Version Analyzed** | |
| **Min SDK** | |
| **Target SDK** | |
| **Obfuscated** | Yes / No / Partial |
| **Decompiler Used** | JADX / JEB / Other |

---

## Protection Mechanisms

<!-- What security measures does the app use? -->

| Protection | Present | Location/Notes |
|------------|---------|----------------|
| SSL Pinning | Yes/No | |
| Root Detection | Yes/No | |
| Emulator Detection | Yes/No | |
| Integrity Checks | Yes/No | |
| Code Obfuscation | Yes/No | |
| Native Libraries | Yes/No | |

---

## Key Classes

<!-- Important classes you've identified -->

| Class | Purpose | Confidence |
|-------|---------|------------|
| | | High/Med/Low |
| | | High/Med/Low |
| | | High/Med/Low |

---

## Hook Points

<!-- Methods you plan to hook -->

### Hook 1: [Purpose]

```java
// Class: com.example.ClassName
// Method signature:
void methodName(String param1, int param2)
```

**Why hook this:** [Reason]
**Expected behavior change:** [What hooking achieves]

### Hook 2: [Purpose]

```java
// Class:
// Method signature:

```

**Why hook this:**
**Expected behavior change:**

---

## String Analysis

<!-- Interesting strings found in the APK -->

| String | Location | Significance |
|--------|----------|--------------|
| | | |
| | | |

---

## Network Endpoints

<!-- API endpoints the app communicates with -->

| Endpoint | Purpose | Auth Required |
|----------|---------|---------------|
| | | |
| | | |

---

## Database/Storage

<!-- How the app stores data locally -->

| Storage Type | Location | Contents |
|--------------|----------|----------|
| SharedPrefs | | |
| SQLite | | |
| Files | | |

---

## Observations

<!-- Free-form notes about app behavior -->

### Startup Sequence

```
1. [What happens first]
2. [What happens next]
3. [Where our hooks could intercept]
```

### Interesting Behaviors

- [Observation 1]
- [Observation 2]

---

## Questions / Unknowns

<!-- What you still need to figure out -->

- [ ] [Question 1]
- [ ] [Question 2]

---

## References

<!-- Links to related resources -->

- [Link] - [Description]
- [Link] - [Description]
