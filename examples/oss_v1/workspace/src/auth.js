// Mock repo workspace: src/auth.js
function requireAuth(req, res, next) {
  if (!req.headers.authorization) {
    return res.status(401).send('Unauthorized');
  }

  next();
}

module.exports = { requireAuth };

// FEATURE: add-rbac
function requireRole(role) {
  return function roleGuard(req, res, next) {
    if (!req.user || req.user.role !== role) {
      return res.status(403).send('Forbidden');
    }

    next();
  };
}

// FEATURE: add-rate-limit
function authRateLimit(req, res, next) {
  req.rateLimit = { max: 10, windowMs: 60_000 };
  next();
}
