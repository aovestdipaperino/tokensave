#version 450

// Maximum number of lights in the scene
#define MAX_LIGHTS 16

// Vertex attributes
in vec3 aPosition;
in vec3 aNormal;
in vec2 aTexCoord;

// Outputs to fragment shader
out vec3 vWorldPos;
out vec3 vNormal;
out vec2 vTexCoord;

// Uniforms
uniform mat4 uModelMatrix;
uniform mat4 uViewMatrix;
uniform mat4 uProjectionMatrix;
uniform float uTime;

/// A point light source in the scene.
struct PointLight {
    vec3 position;
    vec3 color;
    float intensity;
    float radius;
};

/// Material surface properties.
struct Material {
    vec3 albedo;
    float metallic;
    float roughness;
};

uniform PointLight uLights[MAX_LIGHTS];
uniform int uNumLights;
uniform Material uMaterial;

// Constant for PI
const float PI = 3.14159265359;

/// Compute the Fresnel-Schlick approximation.
vec3 fresnelSchlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(1.0 - cosTheta, 5.0);
}

/// Normal distribution function (GGX/Trowbridge-Reitz).
float distributionGGX(vec3 N, vec3 H, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float NdotH = max(dot(N, H), 0.0);
    float NdotH2 = NdotH * NdotH;

    float denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;

    return a2 / denom;
}

/// Geometry function using Schlick-GGX.
float geometrySchlickGGX(float NdotV, float roughness) {
    float r = roughness + 1.0;
    float k = (r * r) / 8.0;
    return NdotV / (NdotV * (1.0 - k) + k);
}

/// Calculate the lighting contribution from a single point light.
vec3 calculatePointLight(PointLight light, vec3 N, vec3 V, vec3 worldPos) {
    vec3 L = normalize(light.position - worldPos);
    vec3 H = normalize(V + L);

    float distance = length(light.position - worldPos);
    if (distance > light.radius) {
        return vec3(0.0);
    }

    float attenuation = 1.0 / (distance * distance);
    vec3 radiance = light.color * light.intensity * attenuation;

    float NDF = distributionGGX(N, H, uMaterial.roughness);
    float G = geometrySchlickGGX(max(dot(N, V), 0.0), uMaterial.roughness);
    vec3 F = fresnelSchlick(max(dot(H, V), 0.0), mix(vec3(0.04), uMaterial.albedo, uMaterial.metallic));

    vec3 numerator = NDF * G * F;
    float denominator = 4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0) + 0.0001;
    vec3 specular = numerator / denominator;

    vec3 kD = (vec3(1.0) - F) * (1.0 - uMaterial.metallic);
    float NdotL = max(dot(N, L), 0.0);

    return (kD * uMaterial.albedo / PI + specular) * radiance * NdotL;
}

/// Main fragment entry point.
void main() {
    vec3 N = normalize(vNormal);
    vec3 V = normalize(-vWorldPos);

    vec3 color = vec3(0.0);
    for (int i = 0; i < uNumLights; i++) {
        color += calculatePointLight(uLights[i], N, V, vWorldPos);
    }

    // Ambient term
    vec3 ambient = vec3(0.03) * uMaterial.albedo;
    color += ambient;

    gl_FragColor = vec4(color, 1.0);
}
