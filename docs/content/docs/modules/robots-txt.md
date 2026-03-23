---
title: Robots.txt Module
description: Learn how to configure the robots.txt handling module
---

# Robots.txt Module

The Robots.txt module serves custom `robots.txt` content to web crawlers and bots. It allows you to control which parts of your site should be accessible to search engine crawlers.

### Request Flow

Request for `/robots.txt` with GET method and configured content → returns 200 with `text/plain`. Otherwise passes through.

## Configuration Options

The Robots.txt module supports the following configuration options:

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `content` | string | No | (none) | The robots.txt content to serve |

### content

The `content` option specifies the robots.txt content to serve:

- **Type**: String
- **Required**: No (if not set, passes through to backend)
- **Format**: Standard robots.txt format

If not configured, the request passes through to the backend service.

## Standard Robots.txt Format

```txt
User-agent: *
Disallow: /private/
Disallow: /admin/
Allow: /public/

Sitemap: https://example.com/sitemap.xml
```

## Example YAML Configuration

### File-Based Configuration

```yaml
modules:
  - name: robots-txt
    type: RobotsTxt
    weight: 40
    global: false
    config:
      content: |
        User-agent: *
        Disallow: /api/
        Disallow: /admin/
        Disallow: /private/
        Allow: /public/
        
        User-agent: Googlebot
        Disallow: /private/
        
        Sitemap: https://example.com/sitemap.xml

ingresses:
  - name: website
    host: www.example.com
    modules:
      - robots-txt
    paths:
      - path: /
        type: prefix
        backend: service
        service:
          name: web-backend
          port: 80
```

### Kubernetes Configuration

First, create a `ModuleConfiguration` resource:

```yaml
apiVersion: modules.ksbh.rs/v1
kind: ModuleConfiguration
metadata:
  name: robots-txt
spec:
  name: robots-txt
  type: RobotsDotTXT
  weight: 40
  global: false
  config:
    content: |
      User-agent: *
      Disallow: /api/
      Disallow: /admin/
      Sitemap: https://example.com/sitemap.xml
```

Then reference it in your ingress:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: website
  annotations:
    modules.ksbh.rs/modules: "robots-txt"
spec:
  ingressClassName: ksbh
  rules:
    - host: www.example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: web-backend
                port:
                  number: 80
```

## Module Properties

- **Type Code**: `RobotsDotTXT` (file provider accepts: `RobotsDotTXT`, `robots.txt`, `robots_txt`, `robotstxt`, `robotsdottxt`; Kubernetes requires exactly: `RobotsDotTXT`)
- **Weight**: explicit per module instance
- **Requires Proper Request**: Yes
- **Requires Body**: No
- **Responds to**: GET requests to `/robots.txt`



### Basic SEO Configuration

```yaml
modules:
  - name: seo-robots
    type: RobotsTxt
    weight: 40
    global: false
    config:
      content: |
        User-agent: *
        Disallow: /cgi-bin/
        Disallow: /tmp/
        Disallow: /wp-admin/
        Allow: /wp-admin/admin-ajax.php
        
        Sitemap: https://example.com/sitemap.xml
```

### Multiple User Agents

```yaml
modules:
  - name: multi-agent-robots
    type: RobotsTxt
    weight: 40
    global: false
    config:
      content: |
        # Default - block everything sensitive
        User-agent: *
        Disallow: /api/
        Disallow: /admin/
        
        # Googlebot can access more
        User-agent: Googlebot
        Disallow: /private/
        Disallow: /legacy/
        
        # Bingbot
        User-agent: Bingbot
        Disallow: /private/
        
        # Allow all for privacy-unconcerned bots
        User-agent: *
        Allow: /
```

## Notes

- ordering is controlled by explicit `weight`
- Only responds to exact `/robots.txt` path (not `/robots.txt/`)
- Content type is `text/plain`
- Without configured content, passes through to backend
